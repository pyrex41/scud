# SCUD Orchestration Improvements: SQLite + Salvo Worktrees

## Overview

This plan implements three major improvements to SCUD's orchestration capabilities, inspired by Gas Town concepts but simplified for SCUD's needs:

1. **SQLite Database** - Replace JSONL event logging with queryable SQLite storage for events, transcripts, and session history
2. **Real-time Transcript Capture** - Auto-import Claude Code conversation logs during swarm execution
3. **Salvo Worktrees** - Automatically provision isolated git worktrees per-tag when swarming, with focused task views

## Current State Analysis

### Storage Architecture
- All data stored in `.scud/` directory relative to project root
- Tasks stored in `.scud/tasks/tasks.scg` (custom SCG format)
- Events logged to `.scud/swarm/events/*.jsonl` (beads mode only)
- Session state in `.scud/swarm/*.json`
- No SQLite - only file-based storage with file locking

### Key Discoveries
- EventWriter at `scud-cli/src/commands/swarm/events.rs:168-215` handles JSONL writing
- Wave mode (`mod.rs`) doesn't log events at all - only beads mode does
- Transcript parsing at `transcript.rs:287-451` reads Claude Code's `~/.claude/projects/` files
- Active tag stored in `.scud/active-tag` - not worktree-aware
- Session locks at `.scud/swarm/{tag}.lock` prevent concurrent swarms on same tag

### Worktree Limitations
- `project_root` determined by `std::env::current_dir()` or `--project` flag
- All worktrees would share `.scud/` if in same repo location
- No mechanism to filter tasks by tag for focused views

## Desired End State

After implementation:

1. **SQLite Database** (`scud.db`):
   - All events, transcripts, sessions stored in queryable SQLite
   - Full text search across transcripts
   - JOIN queries for "show all tool calls for task X"
   - Aggregate stats: tokens used, duration, success rates
   - Timeline reconstruction for future Descartes replay

2. **Real-time Transcript Capture**:
   - Swarm execution automatically imports agent transcripts
   - Background watcher catches transcripts from manual agent sessions
   - Transcripts linked to tasks via session correlation

3. **Salvo Worktrees** (automatic):
   - `scud swarm --tag backend` auto-provisions worktree at `../<project>.salvo.backend/`
   - Worktree gets filtered task file with only that salvo's tasks
   - Other salvos shown as collapsed headers (no task noise)
   - Parallel swarms on different tags run in different worktrees without conflicts
   - On swarm completion, task status changes auto-sync back to main branch
   - Worktrees reused on subsequent runs, cleaned up with `scud salvo remove`

### Verification
- `scud transcript search "error"` returns matching transcript entries
- `scud retro --session X` shows full timeline from SQLite
- `scud swarm --tag backend` auto-creates worktree and runs
- Two terminals can run `scud swarm --tag backend` and `scud swarm --tag frontend` simultaneously (auto-worktrees)
- `scud stats` shows aggregate data from SQLite

## What We're NOT Doing

- Military theming rename (separate effort if desired)
- Descartes GUI replay integration (future work, depends on SQLite)
- Merge queue / refinery concept (worktree isolation may eliminate need)
- JSONL backwards compatibility (full migration to SQLite)
- Remote/federated workers (future work)

## Implementation Approach

We'll implement in three phases, each building on the previous:
1. SQLite foundation - schema, initialization, EventWriter migration
2. Transcript capture - real-time import during swarm, background daemon
3. Salvo worktrees - automatic provisioning on swarm, filtered views, auto-sync back

---

## Phase 1: SQLite Foundation

### Overview
Add SQLite database to SCUD, migrate event logging from JSONL to SQLite, and establish the schema for all future data storage needs.

### Changes Required

#### 1.1 Add Dependencies

**File**: `scud-cli/Cargo.toml`
**Changes**: Add rusqlite with bundled SQLite

```toml
[dependencies]
# ... existing deps ...
rusqlite = { version = "0.32", features = ["bundled"] }
```

#### 1.2 Create Database Module

**File**: `scud-cli/src/db/mod.rs` (new file)
**Changes**: Database initialization and connection management

```rust
use anyhow::Result;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub mod schema;
pub mod events;
pub mod transcripts;
pub mod sessions;

/// Database connection wrapper with lazy initialization
pub struct Database {
    path: PathBuf,
    conn: Mutex<Option<Connection>>,
}

impl Database {
    pub fn new(project_root: &Path) -> Self {
        let path = project_root.join(".scud").join("scud.db");
        Self {
            path,
            conn: Mutex::new(None),
        }
    }

    /// Get or create database connection
    pub fn connection(&self) -> Result<std::sync::MutexGuard<Option<Connection>>> {
        let mut guard = self.conn.lock().unwrap();
        if guard.is_none() {
            let conn = Connection::open_with_flags(
                &self.path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
            )?;
            // Enable WAL mode for better concurrent access
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
            *guard = Some(conn);
        }
        Ok(guard)
    }

    /// Initialize database with schema
    pub fn initialize(&self) -> Result<()> {
        let guard = self.connection()?;
        let conn = guard.as_ref().unwrap();
        schema::create_tables(conn)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
```

#### 1.3 Create Database Schema

**File**: `scud-cli/src/db/schema.rs` (new file)
**Changes**: Define all tables

```rust
use anyhow::Result;
use rusqlite::Connection;

pub fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(r#"
        -- Swarm/spawn sessions
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            session_name TEXT NOT NULL,
            tag TEXT NOT NULL,
            terminal_mode TEXT NOT NULL,  -- tmux, extensions, beads, server
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
            exit_code INTEGER,
            FOREIGN KEY (session_id) REFERENCES sessions(session_id)
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
            kind TEXT NOT NULL,  -- spawned, completed, tool_call, file_read, etc.
            success INTEGER,
            duration_ms INTEGER,
            tool_name TEXT,
            file_path TEXT,
            dependency_id TEXT,
            reason TEXT,
            data TEXT,  -- JSON for additional fields
            FOREIGN KEY (session_id) REFERENCES sessions(session_id),
            FOREIGN KEY (agent_run_id) REFERENCES agent_runs(id)
        );
        CREATE INDEX IF NOT EXISTS idx_events_session_task ON events(session_id, task_id);
        CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
        CREATE INDEX IF NOT EXISTS idx_events_kind ON events(kind);

        -- Transcript messages (from Claude Code JSONL)
        CREATE TABLE IF NOT EXISTS transcript_messages (
            id INTEGER PRIMARY KEY,
            claude_session_id TEXT NOT NULL,  -- Claude's session UUID
            scud_session_id TEXT,  -- Link to our session if known
            task_id TEXT,  -- Link to task if known
            timestamp TEXT NOT NULL,
            uuid TEXT NOT NULL,
            parent_uuid TEXT,
            role TEXT NOT NULL,  -- user/assistant
            content TEXT NOT NULL,  -- Full content (text or JSON)
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
            tool_id TEXT NOT NULL,  -- Claude's tool_use ID
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
    "#)?;
    Ok(())
}
```

#### 1.4 Create Events DB Module

**File**: `scud-cli/src/db/events.rs` (new file)
**Changes**: Event insertion and querying

```rust
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde_json::Value;

use crate::commands::swarm::events::{AgentEvent, EventKind};

pub fn insert_event(conn: &Connection, event: &AgentEvent) -> Result<i64> {
    let (kind, success, duration_ms, tool_name, file_path, dependency_id, reason, data) =
        match &event.event {
            EventKind::Spawned => ("spawned", None, None, None, None, None, None, None),
            EventKind::Started => ("started", None, None, None, None, None, None, None),
            EventKind::Completed { success, duration_ms } => {
                ("completed", Some(*success as i32), Some(*duration_ms as i64), None, None, None, None, None)
            }
            EventKind::Failed { reason } => {
                ("failed", Some(0), None, None, None, None, Some(reason.as_str()), None)
            }
            EventKind::ToolCall { tool, input_summary } => {
                ("tool_call", None, None, Some(tool.as_str()), None, None, None,
                 input_summary.as_ref().map(|s| serde_json::json!({"input_summary": s}).to_string()))
            }
            EventKind::ToolResult { tool, success, duration_ms } => {
                ("tool_result", Some(*success as i32), duration_ms.map(|d| d as i64),
                 Some(tool.as_str()), None, None, None, None)
            }
            EventKind::FileRead { path } => {
                ("file_read", None, None, None, Some(path.as_str()), None, None, None)
            }
            EventKind::FileWrite { path, lines_changed } => {
                ("file_write", None, None, None, Some(path.as_str()), None, None,
                 lines_changed.map(|l| serde_json::json!({"lines_changed": l}).to_string()))
            }
            EventKind::DependencyMet { dependency_id } => {
                ("dependency_met", None, None, None, None, Some(dependency_id.as_str()), None, None)
            }
            EventKind::Unblocked { by_task_id } => {
                ("unblocked", None, None, None, None, Some(by_task_id.as_str()), None, None)
            }
            EventKind::Output { line } => {
                ("output", None, None, None, None, None, None, Some(serde_json::json!({"line": line}).to_string()))
            }
            EventKind::Custom { name, data } => {
                ("custom", None, None, None, None, None, Some(name.as_str()),
                 data.as_ref().map(|d| d.to_string()))
            }
        };

    conn.execute(
        "INSERT INTO events (timestamp, session_id, task_id, kind, success, duration_ms,
         tool_name, file_path, dependency_id, reason, data)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            event.timestamp.to_rfc3339(),
            event.session_id,
            event.task_id,
            kind,
            success,
            duration_ms,
            tool_name,
            file_path,
            dependency_id,
            reason,
            data,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn get_events_for_session(conn: &Connection, session_id: &str) -> Result<Vec<AgentEvent>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, session_id, task_id, kind, success, duration_ms,
                tool_name, file_path, dependency_id, reason, data
         FROM events WHERE session_id = ? ORDER BY timestamp ASC"
    )?;

    let events = stmt.query_map(params![session_id], |row| {
        let timestamp: String = row.get(0)?;
        let session_id: String = row.get(1)?;
        let task_id: String = row.get(2)?;
        let kind: String = row.get(3)?;
        let success: Option<i32> = row.get(4)?;
        let duration_ms: Option<i64> = row.get(5)?;
        let tool_name: Option<String> = row.get(6)?;
        let file_path: Option<String> = row.get(7)?;
        let dependency_id: Option<String> = row.get(8)?;
        let reason: Option<String> = row.get(9)?;
        let data: Option<String> = row.get(10)?;

        let event = match kind.as_str() {
            "spawned" => EventKind::Spawned,
            "started" => EventKind::Started,
            "completed" => EventKind::Completed {
                success: success.unwrap_or(0) != 0,
                duration_ms: duration_ms.unwrap_or(0) as u64,
            },
            "failed" => EventKind::Failed {
                reason: reason.unwrap_or_default(),
            },
            "tool_call" => EventKind::ToolCall {
                tool: tool_name.unwrap_or_default(),
                input_summary: data.and_then(|d| {
                    serde_json::from_str::<Value>(&d).ok()
                        .and_then(|v| v.get("input_summary").and_then(|s| s.as_str()).map(String::from))
                }),
            },
            "unblocked" => EventKind::Unblocked {
                by_task_id: dependency_id.unwrap_or_default(),
            },
            _ => EventKind::Custom {
                name: kind.clone(),
                data: data.and_then(|d| serde_json::from_str(&d).ok()),
            },
        };

        Ok(AgentEvent {
            timestamp: DateTime::parse_from_rfc3339(&timestamp)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            session_id,
            task_id,
            event,
        })
    })?;

    events.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}
```

#### 1.5 Update EventWriter for SQLite

**File**: `scud-cli/src/commands/swarm/events.rs`
**Changes**: Replace JSONL writing with SQLite insertion

```rust
// Add to imports at top of file
use crate::db::Database;

// Modify EventWriter struct (lines 139-142)
pub struct EventWriter {
    session_id: String,
    db: Arc<Database>,
}

impl EventWriter {
    // Modify new() (lines 145-153)
    pub fn new(project_root: &Path, session_id: &str) -> Result<Self> {
        let db = Arc::new(Database::new(project_root));
        db.initialize()?; // Ensure tables exist

        Ok(Self {
            session_id: session_id.to_string(),
            db,
        })
    }

    // Replace write() (lines 168-178)
    pub fn write(&self, event: &AgentEvent) -> Result<()> {
        let guard = self.db.connection()?;
        let conn = guard.as_ref().unwrap();
        crate::db::events::insert_event(conn, event)?;
        Ok(())
    }

    // Remove write_with_task_log() - no longer needed with SQLite
    // Task-specific queries done via SQL WHERE clause

    // Keep convenience methods, they now use write() which goes to SQLite
}

// Modify EventReader (lines 219-322)
pub struct EventReader {
    db: Arc<Database>,
}

impl EventReader {
    pub fn new(project_root: &Path) -> Result<Self> {
        let db = Arc::new(Database::new(project_root));
        Ok(Self { db })
    }

    pub fn load_session(&self, session_id: &str) -> Result<Vec<AgentEvent>> {
        let guard = self.db.connection()?;
        let conn = guard.as_ref().unwrap();
        crate::db::events::get_events_for_session(conn, session_id)
    }

    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let guard = self.db.connection()?;
        let conn = guard.as_ref().unwrap();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT session_id FROM sessions ORDER BY started_at DESC"
        )?;
        let sessions = stmt.query_map([], |row| row.get(0))?;
        sessions.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}
```

#### 1.6 Update Storage Initialization

**File**: `scud-cli/src/storage/mod.rs`
**Changes**: Initialize SQLite database during `scud init`

```rust
// Add to initialize_with_config() after line 245
// Initialize SQLite database
let db = crate::db::Database::new(&self.project_root);
db.initialize()?;
```

#### 1.7 Add Wave Mode Event Logging

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add event logging to wave mode (currently missing)

At line ~92, add EventWriter initialization:
```rust
let event_writer = EventWriter::new(&working_dir, &session_name)?;
```

In `execute_round()` at line ~980, after successful spawn:
```rust
if let Err(e) = event_writer.log_spawned(&info.task.id) {
    eprintln!("Warning: Failed to log spawn event: {}", e);
}
```

In `wait_for_round_completion()`, add completion event logging when status changes detected.

### Success Criteria

#### Automated Verification:
- [x] `cargo build` succeeds with rusqlite dependency
- [x] `cargo test` passes (existing tests still work)
- [x] `scud init` creates `.scud/scud.db` file
- [x] `sqlite3 .scud/scud.db ".tables"` shows all expected tables
- [x] `scud swarm --tag test --dry-run` completes without errors
- [x] Event logging works in both beads and wave modes

#### Manual Verification:
- [x] Run `scud swarm` on a small tag, verify events appear in SQLite
- [x] Run `scud retro` and confirm it reads from SQLite
- [x] Query database directly: `sqlite3 .scud/scud.db "SELECT * FROM events LIMIT 5"`
- [x] Verify database size is reasonable after a swarm run

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 2: Real-time Transcript Capture

### Overview
Automatically import Claude Code conversation transcripts during swarm execution, linking them to tasks and sessions for queryable history.

### Changes Required

#### 2.1 Create Transcripts DB Module

**File**: `scud-cli/src/db/transcripts.rs` (new file)
**Changes**: Transcript insertion and querying

```rust
use anyhow::Result;
use rusqlite::{params, Connection};
use crate::commands::swarm::transcript::{Transcript, TranscriptMessage, ToolCall, ToolResult};

pub fn insert_transcript(
    conn: &Connection,
    transcript: &Transcript,
    scud_session_id: Option<&str>,
    task_id: Option<&str>,
) -> Result<()> {
    for msg in &transcript.messages {
        let content = match &msg.content {
            crate::commands::swarm::transcript::MessageContent::Text(t) => t.clone(),
            crate::commands::swarm::transcript::MessageContent::Structured(s) => {
                serde_json::to_string(s)?
            }
        };

        conn.execute(
            "INSERT INTO transcript_messages
             (claude_session_id, scud_session_id, task_id, timestamp, uuid, parent_uuid,
              role, content, model, input_tokens, output_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                transcript.session_id,
                scud_session_id,
                task_id,
                msg.timestamp.to_rfc3339(),
                msg.uuid,
                msg.parent_uuid,
                msg.role,
                content,
                None::<String>, // model extracted from structured content if needed
                None::<i64>,    // tokens extracted from structured content if needed
                None::<i64>,
            ],
        )?;
        let message_id = conn.last_insert_rowid();

        // Insert tool calls for this message
        for tool_call in &transcript.tool_calls {
            // Only insert if timestamp matches this message
            if tool_call.timestamp == msg.timestamp {
                conn.execute(
                    "INSERT INTO tool_calls
                     (message_id, claude_session_id, timestamp, tool_id, tool_name, input_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        message_id,
                        transcript.session_id,
                        tool_call.timestamp.to_rfc3339(),
                        tool_call.id,
                        tool_call.name,
                        tool_call.input.to_string(),
                    ],
                )?;
            }
        }

        // Insert tool results for this message
        for tool_result in &transcript.tool_results {
            if tool_result.timestamp == msg.timestamp {
                conn.execute(
                    "INSERT INTO tool_results
                     (message_id, claude_session_id, timestamp, tool_use_id, content, is_error)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        message_id,
                        transcript.session_id,
                        tool_result.timestamp.to_rfc3339(),
                        tool_result.tool_use_id,
                        tool_result.content,
                        tool_result.is_error as i32,
                    ],
                )?;
            }
        }
    }
    Ok(())
}

pub fn search_transcripts(conn: &Connection, query: &str) -> Result<Vec<TranscriptSearchResult>> {
    let mut stmt = conn.prepare(
        "SELECT tm.claude_session_id, tm.task_id, tm.timestamp, tm.role,
                substr(tm.content, 1, 200) as content_preview
         FROM transcript_messages tm
         WHERE tm.content LIKE ?1
         ORDER BY tm.timestamp DESC
         LIMIT 100"
    )?;

    let pattern = format!("%{}%", query);
    let results = stmt.query_map(params![pattern], |row| {
        Ok(TranscriptSearchResult {
            session_id: row.get(0)?,
            task_id: row.get(1)?,
            timestamp: row.get(2)?,
            role: row.get(3)?,
            content_preview: row.get(4)?,
        })
    })?;

    results.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[derive(Debug)]
pub struct TranscriptSearchResult {
    pub session_id: String,
    pub task_id: Option<String>,
    pub timestamp: String,
    pub role: String,
    pub content_preview: String,
}

pub fn get_transcript_stats(conn: &Connection) -> Result<TranscriptStats> {
    let total_messages: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transcript_messages", [], |r| r.get(0)
    )?;
    let total_tool_calls: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0)
    )?;
    let total_sessions: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT claude_session_id) FROM transcript_messages", [], |r| r.get(0)
    )?;

    Ok(TranscriptStats {
        total_messages,
        total_tool_calls,
        total_sessions,
    })
}

#[derive(Debug)]
pub struct TranscriptStats {
    pub total_messages: i64,
    pub total_tool_calls: i64,
    pub total_sessions: i64,
}
```

#### 2.2 Create Transcript Watcher

**File**: `scud-cli/src/transcript_watcher.rs` (new file)
**Changes**: Background file watcher for Claude Code transcripts

```rust
use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::commands::swarm::transcript::{find_claude_project_dir, parse_transcript};
use crate::db::Database;

pub struct TranscriptWatcher {
    db: Arc<Database>,
    project_root: PathBuf,
    imported_sessions: Arc<Mutex<HashSet<String>>>,
}

impl TranscriptWatcher {
    pub fn new(project_root: &Path, db: Arc<Database>) -> Self {
        Self {
            db,
            project_root: project_root.to_path_buf(),
            imported_sessions: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Import all transcripts for current project
    pub fn import_all(&self, scud_session_id: Option<&str>, task_id: Option<&str>) -> Result<usize> {
        let claude_dir = find_claude_project_dir(&self.project_root)?;
        let mut count = 0;

        for entry in std::fs::read_dir(&claude_dir)? {
            let path = entry?.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let session_id = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                // Skip if already imported
                {
                    let imported = self.imported_sessions.lock().unwrap();
                    if imported.contains(&session_id) {
                        continue;
                    }
                }

                // Check if already in database
                {
                    let guard = self.db.connection()?;
                    let conn = guard.as_ref().unwrap();
                    let exists: i64 = conn.query_row(
                        "SELECT COUNT(*) FROM transcript_messages WHERE claude_session_id = ?",
                        [&session_id],
                        |r| r.get(0),
                    )?;
                    if exists > 0 {
                        self.imported_sessions.lock().unwrap().insert(session_id);
                        continue;
                    }
                }

                // Parse and import
                if let Ok(transcript) = parse_transcript(&path) {
                    let guard = self.db.connection()?;
                    let conn = guard.as_ref().unwrap();
                    crate::db::transcripts::insert_transcript(
                        conn,
                        &transcript,
                        scud_session_id,
                        task_id,
                    )?;
                    self.imported_sessions.lock().unwrap().insert(session_id);
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Watch for new transcripts and import them
    pub fn watch(&self, scud_session_id: &str) -> Result<()> {
        let claude_dir = find_claude_project_dir(&self.project_root)?;
        let (tx, rx) = channel();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        )?;

        watcher.watch(&claude_dir, RecursiveMode::NonRecursive)?;

        // Process events
        while let Ok(event) = rx.recv() {
            for path in event.paths {
                if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    // Small delay to let file finish writing
                    std::thread::sleep(Duration::from_millis(500));
                    let _ = self.import_file(&path, Some(scud_session_id), None);
                }
            }
        }
        Ok(())
    }

    fn import_file(&self, path: &Path, scud_session_id: Option<&str>, task_id: Option<&str>) -> Result<()> {
        let session_id = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if let Ok(transcript) = parse_transcript(path) {
            let guard = self.db.connection()?;
            let conn = guard.as_ref().unwrap();
            crate::db::transcripts::insert_transcript(conn, &transcript, scud_session_id, task_id)?;
            self.imported_sessions.lock().unwrap().insert(session_id);
        }
        Ok(())
    }
}
```

#### 2.3 Integrate Watcher into Swarm

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Start transcript watcher during swarm execution

Add import at top:
```rust
use crate::transcript_watcher::TranscriptWatcher;
```

After EventWriter initialization (~line 92):
```rust
// Start transcript watcher in background thread
let watcher_db = db.clone();
let watcher_session = session_name.clone();
let watcher_root = working_dir.clone();
let _watcher_handle = std::thread::spawn(move || {
    let watcher = TranscriptWatcher::new(&watcher_root, watcher_db);
    if let Err(e) = watcher.watch(&watcher_session) {
        eprintln!("Transcript watcher error: {}", e);
    }
});
```

#### 2.4 Add Transcript Search Command

**File**: `scud-cli/src/main.rs`
**Changes**: Add `scud transcript search` and `scud transcript stats` subcommands

Add to Commands enum (~line 761):
```rust
#[command(subcommand)]
Transcript(TranscriptCommand),
```

Add subcommand enum:
```rust
#[derive(Subcommand, Clone)]
enum TranscriptCommand {
    /// Search transcript content
    Search {
        /// Search query
        query: String,
    },
    /// Show transcript statistics
    Stats,
    /// Import all transcripts for current project
    Import,
    /// View a specific session transcript
    View {
        /// Session ID
        #[arg(long)]
        session: Option<String>,
        /// Show full transcript
        #[arg(long)]
        full: bool,
    },
    /// List recent transcript sessions
    List,
}
```

Add command handler:
```rust
Commands::Transcript(cmd) => match cmd {
    TranscriptCommand::Search { query } => {
        let db = Database::new(&working_dir);
        let guard = db.connection()?;
        let conn = guard.as_ref().unwrap();
        let results = crate::db::transcripts::search_transcripts(conn, &query)?;
        for r in results {
            println!("{} [{}] {}: {}", r.timestamp, r.task_id.unwrap_or_default(), r.role, r.content_preview);
        }
    }
    TranscriptCommand::Stats => {
        let db = Database::new(&working_dir);
        let guard = db.connection()?;
        let conn = guard.as_ref().unwrap();
        let stats = crate::db::transcripts::get_transcript_stats(conn)?;
        println!("Transcript Statistics:");
        println!("  Sessions: {}", stats.total_sessions);
        println!("  Messages: {}", stats.total_messages);
        println!("  Tool calls: {}", stats.total_tool_calls);
    }
    TranscriptCommand::Import => {
        let db = Arc::new(Database::new(&working_dir));
        let watcher = TranscriptWatcher::new(&working_dir, db);
        let count = watcher.import_all(None, None)?;
        println!("Imported {} transcript sessions", count);
    }
    // ... View and List handlers similar to existing transcript command
}
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build` succeeds with notify dependency
- [x] `cargo test` passes
- [x] `scud transcript import` runs without errors
- [x] `scud transcript stats` shows counts
- [x] `scud transcript search "test"` returns results (if transcripts exist)

#### Manual Verification:
- [ ] Run `scud swarm` on a tag, then check `scud transcript stats` shows new entries (requires live swarm)
- [x] Search for a tool name: `scud transcript search "Read"` finds results
- [x] Query database: `sqlite3 .scud/scud.db "SELECT COUNT(*) FROM transcript_messages"` → 11,879
- [x] Verify transcripts are linked to correct sessions (bulk import has no scud_session_id, expected; watcher sets it during live swarm)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding to the next phase.

---

## Phase 3: Salvo Worktrees (Automatic)

### Overview
When `scud swarm --tag <tag>` is invoked, SCUD automatically provisions a git worktree for that tag, generates a filtered task file, runs the swarm in isolation, and syncs results back. No manual worktree setup required.

### Design

**Automatic lifecycle:**
1. `scud swarm --tag backend` is run from main project directory
2. SCUD checks if a worktree already exists for `backend` (via SQLite lookup)
3. If not, creates one at convention path: `../<project-name>.salvo.<tag>/`
4. Generates filtered task file (full details for target tag, collapsed stubs for others)
5. Copies `.scud/config.toml` and sets active tag
6. Runs the swarm inside the worktree directory
7. On swarm completion, auto-syncs task status changes back to main branch's `tasks.scg`
8. Worktree persists for reuse on next swarm invocation

**Convention path:** Given project at `/home/user/myproject` with tag `backend`:
- Worktree created at `/home/user/myproject.salvo.backend/`
- Branch: `salvo/backend`
- Configurable via `--salvo-dir <path>` override

**Opt-out:** `scud swarm --tag backend --no-worktree` runs in-place (current behavior).

### Changes Required

#### 3.1 Add Salvo Module

**File**: `scud-cli/src/commands/salvo.rs` (new file)
**Changes**: Worktree lifecycle management

```rust
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::db::Database;
use crate::storage::Storage;
use crate::formats::scg::serialize_scg;

/// Resolve or create the worktree for a tag.
/// Returns the worktree path (which becomes the swarm's working directory).
pub fn ensure_worktree(
    project_root: &Path,
    tag: &str,
    custom_path: Option<&Path>,
) -> Result<PathBuf> {
    let db = Database::new(project_root);
    db.initialize()?;

    // Check if worktree already exists in database
    let existing = {
        let guard = db.connection()?;
        let conn = guard.as_ref().unwrap();
        conn.query_row(
            "SELECT worktree_path FROM salvo_worktrees WHERE tag = ?",
            [tag],
            |row| row.get::<_, String>(0),
        ).ok()
    };

    if let Some(existing_path) = existing {
        let wt_path = PathBuf::from(&existing_path);
        if wt_path.exists() {
            // Refresh the filtered task file with latest state
            refresh_filtered_tasks(project_root, &wt_path, tag)?;
            println!("Using existing salvo worktree at {}", wt_path.display());
            return Ok(wt_path);
        }
        // Path recorded but directory gone - clean up stale record and recreate
        let guard = db.connection()?;
        let conn = guard.as_ref().unwrap();
        conn.execute("DELETE FROM salvo_worktrees WHERE tag = ?", [tag])?;
    }

    // Determine worktree path
    let worktree_path = if let Some(p) = custom_path {
        p.to_path_buf()
    } else {
        default_worktree_path(project_root, tag)
    };

    create_worktree(project_root, tag, &worktree_path)?;
    Ok(worktree_path)
}

/// Convention: ../<project-name>.salvo.<tag>/
fn default_worktree_path(project_root: &Path, tag: &str) -> PathBuf {
    let project_name = project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    let parent = project_root.parent().unwrap_or(project_root);
    parent.join(format!("{}.salvo.{}", project_name, tag))
}

/// Create a new worktree for a tag
fn create_worktree(
    project_root: &Path,
    tag: &str,
    worktree_path: &Path,
) -> Result<()> {
    let storage = Storage::new(Some(project_root.to_path_buf()));

    // Verify tag exists
    let phases = storage.load_tasks()?;
    if !phases.contains_key(tag) {
        bail!("Tag '{}' not found. Available tags: {:?}",
              tag, phases.keys().collect::<Vec<_>>());
    }

    // Create git worktree with branch salvo/<tag>
    let branch_name = format!("salvo/{}", tag);
    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch_name])
        .arg(worktree_path)
        .current_dir(project_root)
        .output()?;

    if !output.status.success() {
        // Branch may already exist, try attaching to it
        let output = Command::new("git")
            .args(["worktree", "add"])
            .arg(worktree_path)
            .arg(&branch_name)
            .current_dir(project_root)
            .output()?;

        if !output.status.success() {
            bail!("Failed to create worktree: {}",
                  String::from_utf8_lossy(&output.stderr));
        }
    }

    // Bootstrap .scud in worktree
    let worktree_scud = worktree_path.join(".scud");
    std::fs::create_dir_all(worktree_scud.join("tasks"))?;
    std::fs::create_dir_all(worktree_scud.join("swarm"))?;

    // Generate filtered task file
    generate_filtered_tasks(project_root, worktree_path, tag)?;

    // Set active tag
    std::fs::write(worktree_scud.join("active-tag"), tag)?;

    // Copy config
    let main_config = project_root.join(".scud").join("config.toml");
    if main_config.exists() {
        std::fs::copy(&main_config, worktree_scud.join("config.toml"))?;
    }

    // Copy guidance files if they exist
    let main_guidance = project_root.join(".scud").join("guidance");
    if main_guidance.exists() {
        let wt_guidance = worktree_scud.join("guidance");
        std::fs::create_dir_all(&wt_guidance)?;
        for entry in std::fs::read_dir(&main_guidance)? {
            let entry = entry?;
            if entry.path().is_file() {
                std::fs::copy(entry.path(), wt_guidance.join(entry.file_name()))?;
            }
        }
    }

    // Record in database (use main project's database, not worktree's)
    let db = Database::new(project_root);
    let guard = db.connection()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "INSERT OR REPLACE INTO salvo_worktrees
         (tag, worktree_path, branch_name, created_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        [tag, worktree_path.to_str().unwrap(), &branch_name],
    )?;

    println!("Created salvo worktree for '{}' at {}", tag, worktree_path.display());
    println!("Branch: {}", branch_name);

    Ok(())
}

/// Generate filtered task file: full detail for target tag, collapsed stubs for others
fn generate_filtered_tasks(
    project_root: &Path,
    worktree_path: &Path,
    target_tag: &str,
) -> Result<()> {
    let storage = Storage::new(Some(project_root.to_path_buf()));
    let phases = storage.load_tasks()?;

    let worktree_tasks = worktree_path.join(".scud").join("tasks").join("tasks.scg");
    let mut output = String::new();

    // Target phase gets full serialization
    if let Some(phase) = phases.get(target_tag) {
        output.push_str(&serialize_scg(phase));
    }

    // Other phases shown as collapsed stubs (dependencies visible, tasks hidden)
    for (tag, phase) in &phases {
        if tag != target_tag {
            if !output.is_empty() {
                output.push_str("\n---\n\n");
            }
            output.push_str("# SCUD Graph v1\n");
            output.push_str(&format!("# Phase: {}\n", tag));
            output.push_str(&format!("# [Collapsed - {} tasks, work in main branch]\n\n", phase.tasks.len()));
            output.push_str("@meta\n");
            output.push_str(&format!("name = \"{}\"\n", phase.name));
            output.push_str("\n@nodes\n");
            output.push_str("# Tasks hidden. Run `scud salvo sync` to merge changes.\n");
        }
    }

    std::fs::write(&worktree_tasks, output)?;
    Ok(())
}

/// Refresh filtered tasks (update worktree with latest from main)
fn refresh_filtered_tasks(
    project_root: &Path,
    worktree_path: &Path,
    tag: &str,
) -> Result<()> {
    // Re-generate with latest main state, but preserve worktree's status changes
    // for the target tag by loading worktree's current state first
    let worktree_storage = Storage::new(Some(worktree_path.to_path_buf()));
    let worktree_phases = worktree_storage.load_tasks().ok();

    let main_storage = Storage::new(Some(project_root.to_path_buf()));
    let main_phases = main_storage.load_tasks()?;

    let worktree_tasks = worktree_path.join(".scud").join("tasks").join("tasks.scg");
    let mut output = String::new();

    // For target tag: prefer worktree version (has in-progress status changes)
    // Fall back to main if worktree doesn't have it yet
    if let Some(phase) = worktree_phases
        .as_ref()
        .and_then(|p| p.get(tag))
        .or_else(|| main_phases.get(tag))
    {
        output.push_str(&serialize_scg(phase));
    }

    // Collapsed stubs for other tags (always from main)
    for (other_tag, phase) in &main_phases {
        if other_tag != tag {
            if !output.is_empty() {
                output.push_str("\n---\n\n");
            }
            output.push_str("# SCUD Graph v1\n");
            output.push_str(&format!("# Phase: {}\n", other_tag));
            output.push_str(&format!("# [Collapsed - {} tasks]\n\n", phase.tasks.len()));
            output.push_str("@meta\n");
            output.push_str(&format!("name = \"{}\"\n", phase.name));
            output.push_str("\n@nodes\n");
            output.push_str("# Tasks hidden. Run `scud salvo sync` to merge changes.\n");
        }
    }

    std::fs::write(&worktree_tasks, output)?;
    Ok(())
}

/// Sync task status changes from worktree back to main branch's tasks.scg
pub fn sync_to_main(
    project_root: &Path,
    worktree_path: &Path,
    tag: &str,
) -> Result<()> {
    let worktree_storage = Storage::new(Some(worktree_path.to_path_buf()));
    let worktree_phases = worktree_storage.load_tasks()?;

    let worktree_phase = worktree_phases.get(tag)
        .ok_or_else(|| anyhow::anyhow!("Tag '{}' not found in worktree", tag))?;

    let main_storage = Storage::new(Some(project_root.to_path_buf()));

    // Use update_group for atomic read-modify-write on main
    main_storage.update_group(tag, worktree_phase)?;

    // Record sync time
    let db = Database::new(project_root);
    let guard = db.connection()?;
    let conn = guard.as_ref().unwrap();
    conn.execute(
        "UPDATE salvo_worktrees SET last_sync_at = datetime('now') WHERE tag = ?",
        [tag],
    )?;

    println!("Synced salvo '{}' back to main", tag);
    Ok(())
}

/// List all salvo worktrees
pub fn list_worktrees(project_root: &Path) -> Result<()> {
    let db = Database::new(project_root);
    let guard = db.connection()?;
    let conn = guard.as_ref().unwrap();

    let mut stmt = conn.prepare(
        "SELECT tag, worktree_path, branch_name, created_at, last_sync_at
         FROM salvo_worktrees ORDER BY created_at DESC"
    )?;

    let worktrees = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    println!("Salvo Worktrees:");
    println!("{:<15} {:<40} {:<20} {}", "Tag", "Path", "Branch", "Last Sync");
    println!("{}", "-".repeat(90));

    for wt in worktrees {
        let (tag, path, branch, _created, synced) = wt?;
        let sync_display = synced.unwrap_or_else(|| "never".to_string());
        let exists = Path::new(&path).exists();
        let status = if exists { "" } else { " (missing)" };
        println!("{:<15} {:<40} {:<20} {}{}",
                 tag, path, branch, sync_display, status);
    }

    Ok(())
}

/// Remove a salvo worktree and its git branch
pub fn remove_worktree(project_root: &Path, tag: &str) -> Result<()> {
    let db = Database::new(project_root);
    let guard = db.connection()?;
    let conn = guard.as_ref().unwrap();

    let row: Option<(String, String)> = conn.query_row(
        "SELECT worktree_path, branch_name FROM salvo_worktrees WHERE tag = ?",
        [tag],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok();

    if let Some((path, _branch)) = row {
        // Remove git worktree
        let _ = Command::new("git")
            .args(["worktree", "remove", &path])
            .current_dir(project_root)
            .output();
    }

    conn.execute("DELETE FROM salvo_worktrees WHERE tag = ?", [tag])?;
    println!("Removed salvo worktree for '{}'", tag);
    Ok(())
}
```

#### 3.2 Integrate into Swarm Command

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Auto-provision worktree at swarm start, auto-sync on completion

This is the key integration point. The swarm command gains two new flags and
wraps its execution in worktree lifecycle management.

Add to swarm CLI args:
```rust
/// Skip automatic worktree creation (run in-place)
#[arg(long)]
no_worktree: bool,

/// Custom directory for salvo worktree
#[arg(long)]
salvo_dir: Option<PathBuf>,
```

At the top of the swarm `run()` function, before session lock acquisition:
```rust
// Determine actual working directory (may be a worktree)
let (effective_working_dir, is_salvo_worktree, main_project_root) =
    if !no_worktree && tag.is_some() {
        let tag_name = tag.as_ref().unwrap();
        let wt_path = crate::commands::salvo::ensure_worktree(
            &working_dir,
            tag_name,
            salvo_dir.as_deref(),
        )?;
        (wt_path, true, Some(working_dir.clone()))
    } else {
        (working_dir.clone(), false, None)
    };

// Use effective_working_dir for all subsequent operations
let working_dir = effective_working_dir;
```

At the end of the swarm `run()` function, after swarm completion:
```rust
// Auto-sync worktree results back to main
if is_salvo_worktree {
    if let (Some(main_root), Some(tag_name)) = (&main_project_root, &tag) {
        if let Err(e) = crate::commands::salvo::sync_to_main(
            main_root, &working_dir, tag_name
        ) {
            eprintln!("Warning: Failed to sync salvo back to main: {}", e);
            eprintln!("Run manually: scud salvo sync {}", tag_name);
        }
    }
}
```

#### 3.3 Add Salvo Management Commands to CLI

**File**: `scud-cli/src/main.rs`
**Changes**: Add `scud salvo` subcommands for manual management

```rust
/// Manage salvo worktrees
#[command(subcommand)]
Salvo(SalvoCommand),
```

```rust
#[derive(Subcommand, Clone)]
enum SalvoCommand {
    /// List all salvo worktrees
    List,
    /// Sync a salvo worktree's task status back to main
    Sync {
        /// Tag name of the salvo
        tag: String,
    },
    /// Remove a salvo worktree
    Remove {
        /// Tag name of the salvo
        tag: String,
    },
}
```

Handler:
```rust
Commands::Salvo(cmd) => match cmd {
    SalvoCommand::List => {
        crate::commands::salvo::list_worktrees(&working_dir)?;
    }
    SalvoCommand::Sync { tag } => {
        let db = Database::new(&working_dir);
        let guard = db.connection()?;
        let conn = guard.as_ref().unwrap();
        let path: String = conn.query_row(
            "SELECT worktree_path FROM salvo_worktrees WHERE tag = ?",
            [&tag], |row| row.get(0),
        )?;
        crate::commands::salvo::sync_to_main(
            &working_dir, &PathBuf::from(path), &tag
        )?;
    }
    SalvoCommand::Remove { tag } => {
        crate::commands::salvo::remove_worktree(&working_dir, &tag)?;
    }
}
```

#### 3.4 Update Session Lock for Worktree Awareness

**File**: `scud-cli/src/commands/swarm/session.rs`
**Changes**: Lock files scoped to worktree context

```rust
pub fn lock_file_path(project_root: &Path, tag: &str) -> PathBuf {
    let worktree_id = get_worktree_id(project_root);
    let lock_name = match worktree_id {
        Some(wt_id) => format!("{}-{}.lock", tag, wt_id),
        None => format!("{}.lock", tag),
    };
    swarm_dir(project_root).join(lock_name)
}

/// Detect if we're in a git worktree (has .git file, not .git directory)
fn get_worktree_id(project_root: &Path) -> Option<String> {
    let git_path = project_root.join(".git");
    if git_path.is_file() {
        project_root.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}
```

### User Experience

**Simple case (fully automatic):**
```bash
$ scud swarm --tag backend
Created salvo worktree for 'backend' at ../myproject.salvo.backend/
Branch: salvo/backend
SCUD Swarm Mode
  Tag: backend
  Working dir: ../myproject.salvo.backend/
  ...
[swarm runs to completion]
Synced salvo 'backend' back to main
```

**Second run (reuses existing worktree):**
```bash
$ scud swarm --tag backend
Using existing salvo worktree at ../myproject.salvo.backend/
SCUD Swarm Mode
  ...
```

**Parallel salvos in two terminals:**
```bash
# Terminal 1                          # Terminal 2
$ scud swarm --tag backend            $ scud swarm --tag frontend
Created salvo worktree at             Created salvo worktree at
  ../myproject.salvo.backend/           ../myproject.salvo.frontend/
[runs in parallel, no conflicts]      [runs in parallel, no conflicts]
```

**Opt-out:**
```bash
$ scud swarm --tag backend --no-worktree
# Runs in current directory (existing behavior)
```

**Management:**
```bash
$ scud salvo list
Salvo Worktrees:
Tag             Path                                     Branch               Last Sync
------------------------------------------------------------------------------------------
backend         ../myproject.salvo.backend/               salvo/backend        2026-01-25T14:30:00
frontend        ../myproject.salvo.frontend/              salvo/frontend       never

$ scud salvo sync backend   # Manual sync
$ scud salvo remove backend # Clean up
```

### Success Criteria

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `cargo test` passes
- [x] `scud salvo list` runs without errors
- [x] `scud swarm --tag <tag> --dry-run` provisions worktree and shows dry run
- [x] `scud swarm --tag <tag> --no-worktree --dry-run` skips worktree

#### Manual Verification:
- [x] `scud swarm --tag backend` auto-creates worktree at `../<project>.salvo.backend/`
- [x] Worktree's `tasks.scg` has full details only for target tag
- [x] Other tags show as collapsed stubs with task counts
- [x] Second `scud swarm --tag backend` reuses existing worktree
- [ ] Two parallel swarms on different tags run without conflicts (requires live swarm with agents)
- [ ] Task status changes sync back to main on swarm completion (requires live swarm completion)
- [ ] `scud salvo sync` works for manual sync (requires worktree with status changes)
- [x] `scud salvo remove` cleans up worktree and git branch

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding.

---

## Testing Strategy

### Unit Tests

- `db::events::insert_event` and `get_events_for_session` round-trip test
- `db::transcripts::insert_transcript` and `search_transcripts` test
- `salvo::generate_filtered_tasks` produces correct output
- Session lock worktree ID generation

### Integration Tests

- Full swarm execution with SQLite logging
- Transcript import from mock JSONL files
- Worktree creation, swarm, sync cycle

### Manual Testing Steps

1. Initialize fresh project with `scud init`
2. Verify `scud.db` created with correct schema
3. Run small swarm, check events in SQLite
4. Import transcripts, search for tool names
5. Create worktree for a tag
6. Run parallel swarms in main and worktree
7. Sync worktree changes back

## Performance Considerations

- SQLite WAL mode enabled for concurrent reads during swarm
- Transcript import batched in transactions
- Worktree filtered task file avoids loading unused data
- Background watcher uses polling (2s interval) to avoid CPU spin

## Migration Notes

- Existing JSONL events will NOT be migrated (clean start)
- Old `.scud/swarm/events/*.jsonl` files can be deleted after upgrade
- Database schema includes version table for future migrations

## References

- Gas Town blog post: Inspiration for convoy (salvo) and orchestration concepts
- Research: `thoughts/shared/research/2026-01-25-gastown-scud-orchestration.md` (to be created)
- SQLite documentation: https://www.sqlite.org/wal.html
- notify crate: https://docs.rs/notify/latest/notify/
