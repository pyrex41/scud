//! Swarm event logging and aggregation
//!
//! Captures structured events from agent execution for retrospective analysis.
//! Events are stored in SQLite for queryable access.

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[cfg(feature = "zmq")]
use super::publisher::EventPublisher;
use crate::db::Database;

#[cfg(feature = "zmq")]
use zmq;

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

    // Wave lifecycle events (from orchestrator)
    WaveStarted {
        wave_number: usize,
        task_count: usize,
    },
    WaveCompleted {
        wave_number: usize,
        duration_ms: u64,
    },

    // Validation events (from orchestrator)
    ValidationPassed,
    ValidationFailed {
        failures: Vec<String>,
    },

    // Repair events (from orchestrator)
    RepairStarted {
        attempt: usize,
        task_ids: Vec<String>,
    },
    RepairCompleted {
        attempt: usize,
        success: bool,
    },

    // Heartbeat events (for connection liveness detection)
    Heartbeat,

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
    pub fn tool_call(
        session_id: &str,
        task_id: &str,
        tool: &str,
        input_summary: Option<&str>,
    ) -> Self {
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

/// Writer for appending events to SQLite database
pub struct EventWriter {
    session_id: String,
    db: Option<Database>,
    /// ZMQ publisher for real-time event streaming
    #[cfg(feature = "zmq")]
    zmq_publisher: Option<super::publisher::EventPublisher>,
    /// Flag to indicate if ZMQ is enabled (for when zmq feature is disabled)
    #[cfg(not(feature = "zmq"))]
    #[allow(dead_code)]
    zmq_enabled: bool,
}

impl EventWriter {
    pub fn new(project_root: &Path, session_id: &str) -> Result<Self> {
        // Ensure .scud directory exists
        let scud_dir = project_root.join(".scud");
        std::fs::create_dir_all(&scud_dir)?;

        let db = Database::new(project_root);
        db.initialize()?;

        Ok(Self {
            session_id: session_id.to_string(),
            db: Some(db),
            #[cfg(feature = "zmq")]
            zmq_publisher: None,
            #[cfg(not(feature = "zmq"))]
            zmq_enabled: false,
        })
    }

    /// Create EventWriter with optional ZMQ publishing
    pub fn new_with_zmq(project_root: &Path, session_id: &str, enable_zmq: bool) -> Result<Self> {
        // Ensure .scud directory exists
        let scud_dir = project_root.join(".scud");
        std::fs::create_dir_all(&scud_dir)?;

        let db = Database::new(project_root);

        #[cfg(feature = "zmq")]
        let zmq_publisher = if enable_zmq {
            let session_dir = project_root.join(".scud/swarm").join(session_id);
            match super::publisher::EventPublisher::new(&session_dir) {
                Ok(pub_) => {
                    tracing::info!("ZMQ event publishing enabled for session {}", session_id);
                    Some(pub_)
                }
                Err(e) => {
                    tracing::warn!("Failed to create ZMQ publisher: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            session_id: session_id.to_string(),
            db: Some(db),
            #[cfg(feature = "zmq")]
            zmq_publisher,
            #[cfg(not(feature = "zmq"))]
            zmq_enabled: enable_zmq,
        })
    }

    /// Get the session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the path to the database file (for display purposes)
    pub fn session_file(&self) -> Option<PathBuf> {
        self.db.as_ref().map(|db| db.path().to_path_buf())
    }

    /// Get the ZMQ publisher for control command handling
    #[cfg(feature = "zmq")]
    pub fn zmq_publisher(&self) -> Option<&super::publisher::EventPublisher> {
        self.zmq_publisher.as_ref()
    }

    /// Publish event via ZMQ (non-blocking, best-effort)
    #[cfg(feature = "zmq")]
    fn zmq_publish(&self, event: super::publisher::ZmqEvent) {
        if let Some(ref publisher) = self.zmq_publisher {
            if let Err(e) = publisher.publish(&event) {
                tracing::debug!("ZMQ publish error (non-fatal): {}", e);
            }
        }
    }

    /// No-op ZMQ publish when zmq feature is disabled
    #[cfg(not(feature = "zmq"))]
    #[allow(dead_code)]
    fn zmq_publish(&self, _event: super::publisher::ZmqEvent) {
        // ZMQ not available
    }

    /// Write an event to the database
    pub fn write(&self, event: &AgentEvent) -> Result<()> {
        if let Some(ref db) = self.db {
            let guard = db.connection()?;
            let conn = guard.as_ref().unwrap();
            crate::db::events::insert_event(conn, event)?;
        }

        // Also publish via ZMQ if configured
        self.zmq_publish_event(event);

        Ok(())
    }

    /// Convert AgentEvent to ZmqEvent and publish
    #[cfg(feature = "zmq")]
    fn zmq_publish_event(&self, event: &AgentEvent) {
        use super::publisher::ZmqEvent;

        let zmq_event = match &event.event {
            EventKind::Spawned => Some(ZmqEvent::TaskSpawned {
                task_id: event.task_id.clone(),
            }),
            EventKind::WaveStarted {
                wave_number,
                task_count,
            } => Some(ZmqEvent::WaveStarted {
                wave: *wave_number,
                tasks: vec![], // TODO: could populate if needed
                task_count: *task_count,
            }),
            EventKind::WaveCompleted {
                wave_number,
                duration_ms,
            } => Some(ZmqEvent::WaveCompleted {
                wave: *wave_number,
                duration_ms: Some(*duration_ms),
            }),
            EventKind::ValidationPassed => Some(ZmqEvent::ValidationPassed),
            EventKind::ValidationFailed { failures } => Some(ZmqEvent::ValidationFailed {
                failures: failures.clone(),
            }),
            EventKind::ToolCall {
                tool,
                input_summary,
                ..
            } => Some(ZmqEvent::ToolCall {
                task_id: event.task_id.clone(),
                tool: tool.clone(),
                input_summary: input_summary.clone(),
            }),
            EventKind::ToolResult {
                tool,
                success,
                duration_ms,
                ..
            } => Some(ZmqEvent::ToolResult {
                task_id: event.task_id.clone(),
                tool: tool.clone(),
                success: *success,
                duration_ms: *duration_ms,
            }),
            EventKind::FileRead { path, .. } => Some(ZmqEvent::FileRead {
                task_id: event.task_id.clone(),
                path: path.clone(),
            }),
            EventKind::FileWrite {
                path,
                lines_changed,
                ..
            } => Some(ZmqEvent::FileWrite {
                task_id: event.task_id.clone(),
                path: path.clone(),
                lines_changed: *lines_changed,
            }),
            EventKind::Completed {
                success,
                duration_ms,
            } => Some(ZmqEvent::TaskCompleted {
                task_id: event.task_id.clone(),
                success: *success,
                duration_ms: Some(*duration_ms),
            }),
            EventKind::Heartbeat => Some(ZmqEvent::Heartbeat {
                timestamp: event.timestamp.to_rfc3339(),
            }),
            _ => None, // Other events not published via ZMQ for now
        };

        if let Some(zmq_event) = zmq_event {
            self.zmq_publish(zmq_event);
        }
    }

    /// No-op ZMQ publish when zmq feature is disabled
    #[cfg(not(feature = "zmq"))]
    fn zmq_publish_event(&self, _event: &AgentEvent) {
        // ZMQ not available
    }

    /// Write an event (SQLite stores all events in one table, no separate task log needed)
    pub fn write_with_task_log(&self, event: &AgentEvent) -> Result<()> {
        self.write(event)
    }

    /// Log a spawn event
    pub fn log_spawned(&self, task_id: &str) -> Result<()> {
        self.write(&AgentEvent::spawned(&self.session_id, task_id))
    }

    /// Log a completion event
    pub fn log_completed(&self, task_id: &str, success: bool, duration_ms: u64) -> Result<()> {
        self.write(&AgentEvent::completed(
            &self.session_id,
            task_id,
            success,
            duration_ms,
        ))
    }

    /// Log an unblocked event
    pub fn log_unblocked(&self, task_id: &str, by_task_id: &str) -> Result<()> {
        self.write(&AgentEvent::unblocked(
            &self.session_id,
            task_id,
            by_task_id,
        ))
    }

    /// Log a wave started event
    pub fn log_wave_started(&self, wave_number: usize, task_count: usize) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            &format!("wave:{}", wave_number),
            EventKind::WaveStarted {
                wave_number,
                task_count,
            },
        ))
    }

    /// Log a wave completed event
    pub fn log_wave_completed(&self, wave_number: usize, duration_ms: u64) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            &format!("wave:{}", wave_number),
            EventKind::WaveCompleted {
                wave_number,
                duration_ms,
            },
        ))
    }

    /// Log a validation passed event
    pub fn log_validation_passed(&self) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            "validation",
            EventKind::ValidationPassed,
        ))
    }

    /// Log a validation failed event
    pub fn log_validation_failed(&self, failures: &[String]) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            "validation",
            EventKind::ValidationFailed {
                failures: failures.to_vec(),
            },
        ))
    }

    /// Log a repair started event
    pub fn log_repair_started(&self, attempt: usize, task_ids: &[String]) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            "repair",
            EventKind::RepairStarted {
                attempt,
                task_ids: task_ids.to_vec(),
            },
        ))
    }

    /// Log a repair completed event
    pub fn log_repair_completed(&self, attempt: usize, success: bool) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            "repair",
            EventKind::RepairCompleted { attempt, success },
        ))
    }

    /// Log a heartbeat event
    pub fn log_heartbeat(&self) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            "heartbeat",
            EventKind::Heartbeat,
        ))
    }

    /// Log a swarm started event
    pub fn log_swarm_started(&self, tag: &str, total_waves: usize) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            "swarm",
            EventKind::Custom {
                name: "swarm_started".to_string(),
                data: Some(serde_json::json!({
                    "tag": tag,
                    "total_waves": total_waves
                })),
            },
        ))
    }

    /// Log a swarm completed event
    pub fn log_swarm_completed(&self, success: bool) -> Result<()> {
        self.write(&AgentEvent::new(
            &self.session_id,
            "swarm",
            EventKind::Custom {
                name: "swarm_completed".to_string(),
                data: Some(serde_json::json!({
                    "success": success
                })),
            },
        ))
    }

    /// Publish a ZMQ event directly (for swarm lifecycle events)
    #[cfg(feature = "zmq")]
    pub fn publish_event(&self, event: &super::publisher::ZmqEvent) -> Result<()> {
        if let Some(ref publisher) = self.zmq_publisher {
            publisher.publish(event)?;
        }
        Ok(())
    }

    /// No-op publish_event when zmq feature is disabled
    #[cfg(not(feature = "zmq"))]
    pub fn publish_event(&self, _event: &super::publisher::ZmqEvent) -> Result<()> {
        Ok(())
    }
}

/// Reader for loading events from SQLite database
pub struct EventReader {
    db: Database,
}

impl EventReader {
    pub fn new(project_root: &Path) -> Self {
        Self {
            db: Database::new(project_root),
        }
    }

    /// Load all events for a session
    pub fn load_session(&self, session_id: &str) -> Result<Vec<AgentEvent>> {
        self.load_all_for_session(session_id)
    }

    /// Load all events for a session (already sorted by timestamp in SQLite)
    pub fn load_all_for_session(&self, session_id: &str) -> Result<Vec<AgentEvent>> {
        let guard = self.db.connection()?;
        let conn = guard.as_ref().unwrap();
        crate::db::events::get_events_for_session(conn, session_id)
    }

    /// List available sessions
    pub fn list_sessions(&self) -> Result<Vec<String>> {
        let guard = self.db.connection()?;
        let conn = guard.as_ref().unwrap();
        crate::db::events::list_sessions(conn)
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
                EventKind::Completed {
                    success,
                    duration_ms,
                } => {
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

    println!("  {} {}", "Session:".dimmed(), timeline.session_id.cyan());

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
            println!("    Tools: {}", task.tools_used.join(", ").dimmed());
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

        let task1 = timeline
            .tasks
            .iter()
            .find(|t| t.task_id == "task:1")
            .unwrap();
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
