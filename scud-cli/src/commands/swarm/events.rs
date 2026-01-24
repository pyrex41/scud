//! Swarm event logging and aggregation
//!
//! Captures structured events from agent execution for retrospective analysis.
//! Events are written to JSONL files and can be aggregated into a timeline.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Event kinds that can be logged
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    // Lifecycle events (from orchestrator)
    Spawned,
    Started,
    Completed {
        success: bool,
        duration_ms: u64,
    },
    Failed {
        reason: String,
    },

    // Tool events (from hooks)
    ToolCall {
        tool: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        input_summary: Option<String>,
    },
    ToolResult {
        tool: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },

    // File events (from hooks)
    FileRead {
        path: String,
    },
    FileWrite {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        lines_changed: Option<u32>,
    },

    // Dependency events (from orchestrator)
    DependencyMet {
        dependency_id: String,
    },
    Unblocked {
        by_task_id: String,
    },

    // Output capture
    Output {
        line: String,
    },

    // Custom events
    Custom {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
    },
}

/// A single event in the timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    /// When this event occurred
    pub timestamp: DateTime<Utc>,
    /// The swarm session this belongs to
    pub session_id: String,
    /// Which task/agent generated this event
    pub task_id: String,
    /// The event details
    #[serde(flatten)]
    pub event: EventKind,
}

impl AgentEvent {
    pub fn new(session_id: &str, task_id: &str, event: EventKind) -> Self {
        Self {
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
            task_id: task_id.to_string(),
            event,
        }
    }

    /// Create a spawned event
    pub fn spawned(session_id: &str, task_id: &str) -> Self {
        Self::new(session_id, task_id, EventKind::Spawned)
    }

    /// Create a completed event
    pub fn completed(session_id: &str, task_id: &str, success: bool, duration_ms: u64) -> Self {
        Self::new(
            session_id,
            task_id,
            EventKind::Completed {
                success,
                duration_ms,
            },
        )
    }

    /// Create a tool call event
    pub fn tool_call(session_id: &str, task_id: &str, tool: &str, input_summary: Option<&str>) -> Self {
        Self::new(
            session_id,
            task_id,
            EventKind::ToolCall {
                tool: tool.to_string(),
                input_summary: input_summary.map(String::from),
            },
        )
    }

    /// Create an unblocked event (task was unblocked by another task completing)
    pub fn unblocked(session_id: &str, task_id: &str, by_task_id: &str) -> Self {
        Self::new(
            session_id,
            task_id,
            EventKind::Unblocked {
                by_task_id: by_task_id.to_string(),
            },
        )
    }
}

/// Writer for appending events to a JSONL file
pub struct EventWriter {
    session_id: String,
    events_dir: PathBuf,
}

impl EventWriter {
    pub fn new(project_root: &Path, session_id: &str) -> Result<Self> {
        let events_dir = project_root.join(".scud").join("swarm").join("events");
        fs::create_dir_all(&events_dir)?;

        Ok(Self {
            session_id: session_id.to_string(),
            events_dir,
        })
    }

    /// Get the path to the session event file
    pub fn session_file(&self) -> PathBuf {
        self.events_dir.join(format!("{}.jsonl", self.session_id))
    }

    /// Get the path to a task-specific event file
    pub fn task_file(&self, task_id: &str) -> PathBuf {
        // Sanitize task_id for filename (replace : with -)
        let safe_id = task_id.replace(':', "-");
        self.events_dir.join(format!("{}-{}.jsonl", self.session_id, safe_id))
    }

    /// Write an event to the session log
    pub fn write(&self, event: &AgentEvent) -> Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.session_file())?;

        let line = serde_json::to_string(event)?;
        writeln!(file, "{}", line)?;

        Ok(())
    }

    /// Write an event to both session and task-specific logs
    pub fn write_with_task_log(&self, event: &AgentEvent) -> Result<()> {
        // Write to session log
        self.write(event)?;

        // Write to task-specific log
        let mut task_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.task_file(&event.task_id))?;

        let line = serde_json::to_string(event)?;
        writeln!(task_file, "{}", line)?;

        Ok(())
    }

    /// Log a spawn event
    pub fn log_spawned(&self, task_id: &str) -> Result<()> {
        self.write_with_task_log(&AgentEvent::spawned(&self.session_id, task_id))
    }

    /// Log a completion event
    pub fn log_completed(&self, task_id: &str, success: bool, duration_ms: u64) -> Result<()> {
        self.write_with_task_log(&AgentEvent::completed(
            &self.session_id,
            task_id,
            success,
            duration_ms,
        ))
    }

    /// Log an unblocked event
    pub fn log_unblocked(&self, task_id: &str, by_task_id: &str) -> Result<()> {
        self.write_with_task_log(&AgentEvent::unblocked(&self.session_id, task_id, by_task_id))
    }
}

/// Reader for loading events from JSONL files
pub struct EventReader {
    events_dir: PathBuf,
}

impl EventReader {
    pub fn new(project_root: &Path) -> Self {
        Self {
            events_dir: project_root.join(".scud").join("swarm").join("events"),
        }
    }

    /// Load all events for a session
    pub fn load_session(&self, session_id: &str) -> Result<Vec<AgentEvent>> {
        let file_path = self.events_dir.join(format!("{}.jsonl", session_id));
        self.load_file(&file_path)
    }

    /// Load events from a JSONL file
    pub fn load_file(&self, path: &Path) -> Result<Vec<AgentEvent>> {
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str(&line) {
                Ok(event) => events.push(event),
                Err(e) => {
                    eprintln!("Warning: Failed to parse event: {}", e);
                }
            }
        }

        Ok(events)
    }

    /// Load all events for a session (including task-specific files)
    pub fn load_all_for_session(&self, session_id: &str) -> Result<Vec<AgentEvent>> {
        let mut events = Vec::new();

        // Load from session file
        events.extend(self.load_session(session_id)?);

        // Load from task-specific files
        if self.events_dir.exists() {
            let prefix = format!("{}-", session_id);
            for entry in fs::read_dir(&self.events_dir)? {
                let entry = entry?;
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with(&prefix) && name.ends_with(".jsonl") {
                        events.extend(self.load_file(&path)?);
                    }
                }
            }
        }

        // Sort by timestamp
        events.sort_by_key(|e| e.timestamp);

        // Deduplicate (same timestamp + task_id + event content)
        // We compare the full serialized event to ensure different tool calls
        // or other events with different content are not incorrectly merged
        events.dedup_by(|a, b| {
            a.timestamp == b.timestamp
                && a.task_id == b.task_id
                && serde_json::to_string(&a.event).ok() == serde_json::to_string(&b.event).ok()
        });

        Ok(events)
    }

    /// List available sessions
    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let mut sessions = Vec::new();

        if !self.events_dir.exists() {
            return Ok(sessions);
        }

        for entry in fs::read_dir(&self.events_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Only include main session files (not task-specific ones)
                if name.ends_with(".jsonl") && !name.contains('-') {
                    if let Some(session_id) = name.strip_suffix(".jsonl") {
                        sessions.push(session_id.to_string());
                    }
                }
            }
        }

        sessions.sort();
        Ok(sessions)
    }
}

/// Aggregated timeline for retrospective analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrospectiveTimeline {
    pub session_id: String,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub tasks: Vec<TaskTimeline>,
    pub total_events: usize,
}

/// Timeline for a single task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTimeline {
    pub task_id: String,
    pub spawned_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub success: Option<bool>,
    pub duration_ms: Option<u64>,
    pub tools_used: Vec<String>,
    pub files_read: Vec<String>,
    pub files_written: Vec<String>,
    pub unblocked_by: Vec<String>,
    pub events: Vec<AgentEvent>,
}

impl RetrospectiveTimeline {
    /// Build a timeline from events
    pub fn from_events(session_id: &str, events: Vec<AgentEvent>) -> Self {
        use std::collections::HashMap;

        let mut task_map: HashMap<String, TaskTimeline> = HashMap::new();

        for event in &events {
            let task = task_map
                .entry(event.task_id.clone())
                .or_insert_with(|| TaskTimeline {
                    task_id: event.task_id.clone(),
                    spawned_at: None,
                    completed_at: None,
                    success: None,
                    duration_ms: None,
                    tools_used: Vec::new(),
                    files_read: Vec::new(),
                    files_written: Vec::new(),
                    unblocked_by: Vec::new(),
                    events: Vec::new(),
                });

            task.events.push(event.clone());

            match &event.event {
                EventKind::Spawned => {
                    task.spawned_at = Some(event.timestamp);
                }
                EventKind::Completed { success, duration_ms } => {
                    task.completed_at = Some(event.timestamp);
                    task.success = Some(*success);
                    task.duration_ms = Some(*duration_ms);
                }
                EventKind::ToolCall { tool, .. } => {
                    if !task.tools_used.contains(tool) {
                        task.tools_used.push(tool.clone());
                    }
                }
                EventKind::FileRead { path } => {
                    if !task.files_read.contains(path) {
                        task.files_read.push(path.clone());
                    }
                }
                EventKind::FileWrite { path, .. } => {
                    if !task.files_written.contains(path) {
                        task.files_written.push(path.clone());
                    }
                }
                EventKind::Unblocked { by_task_id } => {
                    if !task.unblocked_by.contains(by_task_id) {
                        task.unblocked_by.push(by_task_id.clone());
                    }
                }
                _ => {}
            }
        }

        let tasks: Vec<TaskTimeline> = task_map.into_values().collect();

        let started_at = events.first().map(|e| e.timestamp);
        let completed_at = events.last().map(|e| e.timestamp);

        Self {
            session_id: session_id.to_string(),
            started_at,
            completed_at,
            tasks,
            total_events: events.len(),
        }
    }

    /// Generate a text summary
    pub fn to_summary(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();

        writeln!(s, "Session: {}", self.session_id).unwrap();
        if let (Some(start), Some(end)) = (self.started_at, self.completed_at) {
            let duration = end.signed_duration_since(start);
            writeln!(s, "Duration: {}s", duration.num_seconds()).unwrap();
        }
        writeln!(s, "Total events: {}", self.total_events).unwrap();
        writeln!(s, "Tasks: {}", self.tasks.len()).unwrap();
        writeln!(s).unwrap();

        for task in &self.tasks {
            writeln!(s, "  [{}]", task.task_id).unwrap();
            if let Some(success) = task.success {
                writeln!(s, "    Status: {}", if success { "✓" } else { "✗" }).unwrap();
            }
            if let Some(duration) = task.duration_ms {
                writeln!(s, "    Duration: {}ms", duration).unwrap();
            }
            if !task.tools_used.is_empty() {
                writeln!(s, "    Tools: {}", task.tools_used.join(", ")).unwrap();
            }
            if !task.files_written.is_empty() {
                writeln!(s, "    Files written: {}", task.files_written.len()).unwrap();
            }
            if !task.unblocked_by.is_empty() {
                writeln!(s, "    Unblocked by: {}", task.unblocked_by.join(", ")).unwrap();
            }
        }

        s
    }
}

/// Print a retrospective for a session
pub fn print_retro(project_root: &Path, session_id: Option<&str>) -> Result<()> {
    use colored::Colorize;

    let reader = EventReader::new(project_root);

    // If no session specified, list available sessions
    let session_id = match session_id {
        Some(id) => id.to_string(),
        None => {
            let sessions = reader.list_sessions()?;
            if sessions.is_empty() {
                println!("{}", "No swarm sessions found.".yellow());
                println!("Run a swarm first: scud swarm --tag <tag>");
                return Ok(());
            }

            println!("{}", "Available sessions:".blue().bold());
            for session in &sessions {
                println!("  • {}", session);
            }

            // Use the most recent session
            if let Some(latest) = sessions.last() {
                println!();
                println!("Showing latest session: {}", latest.cyan());
                latest.clone()
            } else {
                return Ok(());
            }
        }
    };

    // Load events
    let events = reader.load_all_for_session(&session_id)?;

    if events.is_empty() {
        println!("{}", "No events found for this session.".yellow());
        return Ok(());
    }

    // Build timeline
    let timeline = RetrospectiveTimeline::from_events(&session_id, events);

    // Print header
    println!();
    println!("{}", "Swarm Retrospective".blue().bold());
    println!("{}", "═".repeat(60).blue());
    println!();

    println!(
        "  {} {}",
        "Session:".dimmed(),
        timeline.session_id.cyan()
    );

    if let (Some(start), Some(end)) = (timeline.started_at, timeline.completed_at) {
        let duration = end.signed_duration_since(start);
        println!(
            "  {} {}s",
            "Duration:".dimmed(),
            duration.num_seconds().to_string().cyan()
        );
        println!(
            "  {} {}",
            "Started:".dimmed(),
            start.format("%Y-%m-%d %H:%M:%S").to_string().dimmed()
        );
    }

    println!(
        "  {} {}",
        "Events:".dimmed(),
        timeline.total_events.to_string().cyan()
    );
    println!(
        "  {} {}",
        "Tasks:".dimmed(),
        timeline.tasks.len().to_string().cyan()
    );
    println!();

    // Print task details
    println!("{}", "Task Timeline".yellow().bold());
    println!("{}", "─".repeat(60).yellow());

    for task in &timeline.tasks {
        let status_icon = match task.success {
            Some(true) => "✓".green(),
            Some(false) => "✗".red(),
            None => "?".yellow(),
        };

        println!();
        println!("  {} [{}]", status_icon, task.task_id.cyan());

        if let Some(duration) = task.duration_ms {
            println!("    Duration: {}ms", duration.to_string().dimmed());
        }

        if !task.tools_used.is_empty() {
            println!(
                "    Tools: {}",
                task.tools_used.join(", ").dimmed()
            );
        }

        if !task.files_written.is_empty() {
            println!(
                "    Files written: {}",
                task.files_written.len().to_string().dimmed()
            );
            for file in task.files_written.iter().take(5) {
                println!("      • {}", file.dimmed());
            }
            if task.files_written.len() > 5 {
                println!(
                    "      ... and {} more",
                    (task.files_written.len() - 5).to_string().dimmed()
                );
            }
        }

        if !task.unblocked_by.is_empty() {
            println!(
                "    Unblocked by: {}",
                task.unblocked_by.join(", ").dimmed()
            );
        }
    }

    println!();
    Ok(())
}

/// Export retrospective as JSON
pub fn export_retro_json(project_root: &Path, session_id: &str) -> Result<String> {
    let reader = EventReader::new(project_root);
    let events = reader.load_all_for_session(session_id)?;
    let timeline = RetrospectiveTimeline::from_events(session_id, events);
    Ok(serde_json::to_string_pretty(&timeline)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_event_serialization() {
        let event = AgentEvent::spawned("session-1", "task:1");
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("spawned"));
        assert!(json.contains("task:1"));

        let parsed: AgentEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.task_id, "task:1");
    }

    #[test]
    fn test_event_writer_reader() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path();

        let writer = EventWriter::new(project_root, "test-session").unwrap();

        // Write events
        writer.log_spawned("task:1").unwrap();
        writer.log_spawned("task:2").unwrap();
        writer.log_completed("task:1", true, 1000).unwrap();

        // Read events
        let reader = EventReader::new(project_root);
        let events = reader.load_session("test-session").unwrap();

        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_retrospective_timeline() {
        let events = vec![
            AgentEvent::spawned("s1", "task:1"),
            AgentEvent::spawned("s1", "task:2"),
            AgentEvent::tool_call("s1", "task:1", "Read", Some("src/main.rs")),
            AgentEvent::completed("s1", "task:1", true, 5000),
            AgentEvent::unblocked("s1", "task:3", "task:1"),
            AgentEvent::completed("s1", "task:2", true, 3000),
        ];

        let timeline = RetrospectiveTimeline::from_events("s1", events);

        assert_eq!(timeline.tasks.len(), 3); // task:1, task:2, task:3
        assert_eq!(timeline.total_events, 6);

        let task1 = timeline.tasks.iter().find(|t| t.task_id == "task:1").unwrap();
        assert_eq!(task1.success, Some(true));
        assert_eq!(task1.duration_ms, Some(5000));
        assert!(task1.tools_used.contains(&"Read".to_string()));
    }

    #[test]
    fn test_deduplication_preserves_different_tool_calls() {
        use chrono::TimeZone;

        // Create two tool call events with the same timestamp and task_id
        // but different tool names - these should NOT be deduplicated
        let fixed_time = Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap();

        let event1 = AgentEvent {
            timestamp: fixed_time,
            session_id: "s1".to_string(),
            task_id: "task:1".to_string(),
            event: EventKind::ToolCall {
                tool: "Read".to_string(),
                input_summary: Some("file1.rs".to_string()),
            },
        };

        let event2 = AgentEvent {
            timestamp: fixed_time,
            session_id: "s1".to_string(),
            task_id: "task:1".to_string(),
            event: EventKind::ToolCall {
                tool: "Write".to_string(),
                input_summary: Some("file2.rs".to_string()),
            },
        };

        let mut events = vec![event1, event2];

        // Sort and dedup using the same logic as load_all_for_session
        events.sort_by_key(|e| e.timestamp);
        events.dedup_by(|a, b| {
            a.timestamp == b.timestamp
                && a.task_id == b.task_id
                && serde_json::to_string(&a.event).ok() == serde_json::to_string(&b.event).ok()
        });

        // Both events should remain (different tool names)
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_deduplication_removes_true_duplicates() {
        use chrono::TimeZone;

        // Create two identical events - these SHOULD be deduplicated
        let fixed_time = Utc.with_ymd_and_hms(2025, 1, 15, 12, 0, 0).unwrap();

        let event1 = AgentEvent {
            timestamp: fixed_time,
            session_id: "s1".to_string(),
            task_id: "task:1".to_string(),
            event: EventKind::Spawned,
        };

        let event2 = AgentEvent {
            timestamp: fixed_time,
            session_id: "s1".to_string(),
            task_id: "task:1".to_string(),
            event: EventKind::Spawned,
        };

        let mut events = vec![event1, event2];

        events.sort_by_key(|e| e.timestamp);
        events.dedup_by(|a, b| {
            a.timestamp == b.timestamp
                && a.task_id == b.task_id
                && serde_json::to_string(&a.event).ok() == serde_json::to_string(&b.event).ok()
        });

        // Only one event should remain (true duplicate)
        assert_eq!(events.len(), 1);
    }
}
