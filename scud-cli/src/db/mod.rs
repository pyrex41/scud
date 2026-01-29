//! SQLite database for event logging, transcript storage, and session history.

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub mod events;
pub mod schema;
pub mod sessions;
pub mod transcripts;

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
    pub fn connection(&self) -> Result<std::sync::MutexGuard<'_, Option<Connection>>> {
        let mut guard = self.conn.lock().unwrap();
        if guard.is_none() {
            let conn = Connection::open_with_flags(
                &self.path,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
            )?;
            // Enable WAL mode for better concurrent access
            // Note: foreign_keys not enforced - events can be written before session records
            conn.execute_batch("PRAGMA journal_mode=WAL;")?;
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

    /// Get events for a session with optional limits
    pub fn get_events_for_session_limited(
        &self,
        session_id: &str,
        limit: Option<usize>,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<crate::commands::swarm::events::AgentEvent>> {
        let guard = self.connection()?;
        let conn = guard.as_ref().unwrap();
        events::get_events_for_session_limited(conn, session_id, limit, since)
    }
}
