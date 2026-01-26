//! Background file watcher for Claude Code transcripts.
//!
//! Monitors `~/.claude/projects/<project>/` for new JSONL transcript files
//! and imports them into the SQLite database in real-time during swarm execution.

use anyhow::Result;
use notify::{Event, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
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

    /// Import all existing transcripts for the current project.
    /// Returns the number of newly imported sessions.
    pub fn import_all(
        &self,
        scud_session_id: Option<&str>,
        task_id: Option<&str>,
    ) -> Result<usize> {
        let claude_dir = match find_claude_project_dir(&self.project_root) {
            Some(dir) => dir,
            None => return Ok(0),
        };

        let mut count = 0;

        for entry in std::fs::read_dir(&claude_dir)? {
            let path = entry?.path();
            if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                let session_id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();

                // Skip if already imported this session
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
                        self.imported_sessions
                            .lock()
                            .unwrap()
                            .insert(session_id);
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
                    self.imported_sessions
                        .lock()
                        .unwrap()
                        .insert(session_id);
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    /// Watch for new transcript files and import them as they appear.
    /// This blocks the calling thread until the channel is closed.
    pub fn watch(&self, scud_session_id: &str) -> Result<()> {
        let claude_dir = match find_claude_project_dir(&self.project_root) {
            Some(dir) => dir,
            None => {
                // No Claude project dir found - nothing to watch
                return Ok(());
            }
        };

        let (tx, rx) = std::sync::mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })?;

        watcher.watch(&claude_dir, RecursiveMode::NonRecursive)?;

        // Process file change events
        while let Ok(event) = rx.recv() {
            for path in event.paths {
                if path.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    // Small delay to let the file finish writing
                    std::thread::sleep(Duration::from_millis(500));
                    let _ = self.import_file(&path, Some(scud_session_id), None);
                }
            }
        }
        Ok(())
    }

    fn import_file(
        &self,
        path: &Path,
        scud_session_id: Option<&str>,
        task_id: Option<&str>,
    ) -> Result<()> {
        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        if let Ok(transcript) = parse_transcript(path) {
            let guard = self.db.connection()?;
            let conn = guard.as_ref().unwrap();
            // Delete old records for this session before re-importing
            // (transcript files are append-only, so we re-import the full file)
            conn.execute(
                "DELETE FROM transcript_messages WHERE claude_session_id = ?",
                [&session_id],
            )?;
            crate::db::transcripts::insert_transcript(
                conn,
                &transcript,
                scud_session_id,
                task_id,
            )?;
            self.imported_sessions
                .lock()
                .unwrap()
                .insert(session_id);
        }
        Ok(())
    }
}
