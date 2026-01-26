//! Session database operations.
//!
//! Manages swarm session records in SQLite.

use anyhow::Result;
use rusqlite::{params, Connection};

pub fn insert_session(
    conn: &Connection,
    session_id: &str,
    session_name: &str,
    tag: &str,
    terminal_mode: &str,
    working_dir: &str,
    round_size: Option<usize>,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO sessions
         (session_id, session_name, tag, terminal_mode, working_dir, round_size, started_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
        params![
            session_id,
            session_name,
            tag,
            terminal_mode,
            working_dir,
            round_size.map(|r| r as i64),
        ],
    )?;
    Ok(())
}

pub fn complete_session(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET completed_at = datetime('now') WHERE session_id = ?",
        params![session_id],
    )?;
    Ok(())
}
