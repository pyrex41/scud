package db

// Schema for transcript database
const schemaSQL = `
CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    model TEXT,
    provider TEXT,
    started_at TEXT,
    ended_at TEXT,
    status TEXT DEFAULT 'active',
    tag TEXT,
    working_dir TEXT
);

CREATE TABLE IF NOT EXISTS agent_runs (
    id TEXT PRIMARY KEY,
    session_id TEXT REFERENCES sessions(id),
    agent_type TEXT,
    task_id TEXT,
    model TEXT,
    started_at TEXT,
    ended_at TEXT,
    status TEXT,
    exit_code INTEGER
);

CREATE TABLE IF NOT EXISTS transcript_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT REFERENCES sessions(id),
    agent_run_id TEXT REFERENCES agent_runs(id),
    role TEXT,
    content TEXT,
    timestamp TEXT,
    token_count INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS tool_calls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER REFERENCES transcript_messages(id),
    tool_name TEXT,
    input TEXT,
    timestamp TEXT
);

CREATE TABLE IF NOT EXISTS tool_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tool_call_id INTEGER REFERENCES tool_calls(id),
    output TEXT,
    is_error INTEGER DEFAULT 0,
    timestamp TEXT
);

CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT REFERENCES sessions(id),
    type TEXT,
    data TEXT,
    timestamp TEXT
);

CREATE TABLE IF NOT EXISTS validation_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT REFERENCES sessions(id),
    all_passed INTEGER,
    timestamp TEXT
);

CREATE TABLE IF NOT EXISTS validation_commands (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    validation_run_id INTEGER REFERENCES validation_runs(id),
    command TEXT,
    passed INTEGER,
    exit_code INTEGER,
    stdout TEXT,
    stderr TEXT,
    duration_sec REAL
);

CREATE TABLE IF NOT EXISTS migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT
);
`

const currentVersion = 1
