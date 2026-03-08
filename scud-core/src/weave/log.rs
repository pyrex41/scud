//! Event log — read/write events.jsonl for coordinator history.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Write};
use std::path::Path;

use super::event::Event;

/// An event with a timestamp, as stored in events.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedEvent {
    pub timestamp: String,
    pub event: Event,
}

/// Manages the events.jsonl file.
pub struct EventLog {
    pub events: Vec<TimestampedEvent>,
}

impl EventLog {
    pub fn new() -> Self {
        EventLog {
            events: Vec::new(),
        }
    }

    /// Load events from a jsonl file path.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let file =
            std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
        let reader = std::io::BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let te: TimestampedEvent = serde_json::from_str(trimmed)
                .with_context(|| format!("parsing event line: {}", trimmed))?;
            events.push(te);
        }

        Ok(EventLog { events })
    }

    /// Append an event to the log.
    pub fn append(&mut self, event: TimestampedEvent) {
        self.events.push(event);
    }

    /// Write all events to a jsonl file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let mut file = std::fs::File::create(path)
            .with_context(|| format!("creating {}", path.display()))?;
        for te in &self.events {
            let json = serde_json::to_string(te)?;
            writeln!(file, "{}", json)?;
        }
        Ok(())
    }

    /// Rotate: keep only the last N events.
    pub fn rotate(&mut self, keep: usize) {
        if self.events.len() > keep {
            let start = self.events.len() - keep;
            self.events = self.events.split_off(start);
        }
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weave::event::EventKind;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_te(kind: EventKind, agent: &str, ts: &str) -> TimestampedEvent {
        TimestampedEvent {
            timestamp: ts.to_string(),
            event: Event {
                kind,
                agent: Some(agent.to_string()),
                target: None,
                task_id: None,
                metadata: HashMap::new(),
            },
        }
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("events.jsonl");

        let mut log = EventLog::new();
        log.append(make_te(EventKind::FileWrite, "agent-1", "2026-01-01T00:00:00Z"));
        log.append(make_te(EventKind::TestPass, "agent-1", "2026-01-01T00:01:00Z"));
        log.save(&path).unwrap();

        let loaded = EventLog::load(&path).unwrap();
        assert_eq!(loaded.events.len(), 2);
        assert_eq!(loaded.events[0].event.kind, EventKind::FileWrite);
        assert_eq!(loaded.events[1].event.kind, EventKind::TestPass);
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nope.jsonl");
        let log = EventLog::load(&path).unwrap();
        assert!(log.events.is_empty());
    }

    #[test]
    fn test_rotate() {
        let mut log = EventLog::new();
        for i in 0..10 {
            log.append(make_te(
                EventKind::FileWrite,
                "agent-1",
                &format!("2026-01-01T00:{:02}:00Z", i),
            ));
        }
        assert_eq!(log.events.len(), 10);

        log.rotate(5);
        assert_eq!(log.events.len(), 5);
        // Should keep the last 5 (indices 5-9 originally)
        assert!(log.events[0].timestamp.contains("05"));
    }

    #[test]
    fn test_rotate_under_limit() {
        let mut log = EventLog::new();
        log.append(make_te(EventKind::FileWrite, "agent-1", "2026-01-01T00:00:00Z"));
        log.rotate(100);
        assert_eq!(log.events.len(), 1);
    }
}
