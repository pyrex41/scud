//! In-memory storage for streaming agent output
//!
//! Provides thread-safe storage for multiple headless agent sessions,
//! with methods for event storage, output rendering, and session management.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use super::events::{StreamEvent, StreamEventKind};

/// Maximum number of output lines to retain per session (memory limit)
const MAX_OUTPUT_LINES: usize = 10_000;

/// Maximum number of events to retain per session (memory limit)
const MAX_EVENTS: usize = 50_000;

/// Status of a headless session
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Starting,
    Running,
    Completed,
    Failed,
}

/// Stream data for a single agent session
#[derive(Debug)]
pub struct SessionStream {
    /// Unique session ID (from harness)
    pub session_id: String,
    /// Associated task ID
    pub task_id: String,
    /// Tag/phase
    pub tag: String,
    /// All events received (bounded by MAX_EVENTS)
    pub events: Vec<StreamEvent>,
    /// Rendered output lines for display (bounded by MAX_OUTPUT_LINES)
    pub output_lines: Vec<String>,
    /// Current status
    pub status: SessionStatus,
    /// When the session started
    pub started_at: Instant,
    /// Process ID (for interruption)
    pub pid: Option<u32>,
    /// Partial line buffer for incomplete text deltas
    partial_line: String,
}

impl SessionStream {
    pub fn new(task_id: &str, tag: &str) -> Self {
        Self {
            session_id: String::new(),
            task_id: task_id.to_string(),
            tag: tag.to_string(),
            events: Vec::new(),
            output_lines: Vec::new(),
            status: SessionStatus::Starting,
            started_at: Instant::now(),
            pid: None,
            partial_line: String::new(),
        }
    }

    /// Add an event and update output lines
    pub fn push_event(&mut self, mut event: StreamEvent) {
        event.timestamp_ms = self.started_at.elapsed().as_millis() as u64;

        // Update output lines based on event
        match &event.kind {
            StreamEventKind::TextDelta { text } => {
                self.append_text(text);
            }
            StreamEventKind::ToolStart {
                tool_name,
                input_summary,
                ..
            } => {
                // Flush any partial line first
                self.flush_partial_line();
                self.push_line(format!(">> {} {}", tool_name, input_summary));
            }
            StreamEventKind::ToolResult {
                tool_name, success, ..
            } => {
                self.flush_partial_line();
                let status = if *success { "ok" } else { "failed" };
                self.push_line(format!("<< {} {}", tool_name, status));
            }
            StreamEventKind::Complete { success } => {
                self.flush_partial_line();
                self.status = if *success {
                    SessionStatus::Completed
                } else {
                    SessionStatus::Failed
                };
            }
            StreamEventKind::Error { message } => {
                self.flush_partial_line();
                self.push_line(format!("ERROR: {}", message));
                self.status = SessionStatus::Failed;
            }
            StreamEventKind::SessionAssigned { session_id } => {
                self.session_id = session_id.clone();
                self.status = SessionStatus::Running;
            }
        }

        // Transition from Starting to Running on first meaningful event
        if matches!(self.status, SessionStatus::Starting)
            && matches!(
                event.kind,
                StreamEventKind::TextDelta { .. }
                    | StreamEventKind::ToolStart { .. }
                    | StreamEventKind::ToolResult { .. }
            )
        {
            self.status = SessionStatus::Running;
        }

        // Store event with memory limit
        if self.events.len() >= MAX_EVENTS {
            // Remove oldest 10% when limit reached
            let drain_count = MAX_EVENTS / 10;
            self.events.drain(0..drain_count);
        }
        self.events.push(event);
    }

    /// Append text, handling newlines properly
    fn append_text(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                // Complete the current line and start a new one
                let line = std::mem::take(&mut self.partial_line);
                self.push_line(line);
            } else {
                self.partial_line.push(ch);
            }
        }
    }

    /// Flush any remaining partial line as a complete line
    fn flush_partial_line(&mut self) {
        if !self.partial_line.is_empty() {
            let line = std::mem::take(&mut self.partial_line);
            self.push_line(line);
        }
    }

    /// Push a line to output with memory limit
    fn push_line(&mut self, line: String) {
        if self.output_lines.len() >= MAX_OUTPUT_LINES {
            // Remove oldest 10% when limit reached
            let drain_count = MAX_OUTPUT_LINES / 10;
            self.output_lines.drain(0..drain_count);
        }
        self.output_lines.push(line);
    }

    /// Get the last N output lines
    pub fn tail(&self, n: usize) -> &[String] {
        let start = self.output_lines.len().saturating_sub(n);
        &self.output_lines[start..]
    }

    /// Get all output lines including any partial line in progress
    pub fn get_all_output(&self) -> Vec<String> {
        let mut lines = self.output_lines.clone();
        if !self.partial_line.is_empty() {
            lines.push(self.partial_line.clone());
        }
        lines
    }

    /// Check if session is still active
    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Starting | SessionStatus::Running)
    }

    /// Get the event count
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Get the output line count
    pub fn line_count(&self) -> usize {
        self.output_lines.len()
    }
}

/// Thread-safe store for multiple agent sessions
#[derive(Debug, Clone, Default)]
pub struct StreamStore {
    sessions: Arc<RwLock<HashMap<String, SessionStream>>>,
}

impl StreamStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new session for a task
    pub fn create_session(&self, task_id: &str, tag: &str) -> String {
        let mut sessions = self.sessions.write().unwrap();
        let stream = SessionStream::new(task_id, tag);
        let key = task_id.to_string();
        sessions.insert(key.clone(), stream);
        key
    }

    /// Push an event to a session
    pub fn push_event(&self, task_id: &str, event: StreamEvent) {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(stream) = sessions.get_mut(task_id) {
            stream.push_event(event);
        }
    }

    /// Set the harness session ID for a task
    pub fn set_session_id(&self, task_id: &str, session_id: &str) {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(stream) = sessions.get_mut(task_id) {
            stream.session_id = session_id.to_string();
            stream.status = SessionStatus::Running;
        }
    }

    /// Set the process ID for a task
    pub fn set_pid(&self, task_id: &str, pid: u32) {
        let mut sessions = self.sessions.write().unwrap();
        if let Some(stream) = sessions.get_mut(task_id) {
            stream.pid = Some(pid);
        }
    }

    /// Get output lines for a task
    pub fn get_output(&self, task_id: &str, limit: usize) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(task_id)
            .map(|s| s.tail(limit).to_vec())
            .unwrap_or_default()
    }

    /// Get all output lines for a task, including any partial line
    pub fn get_all_output(&self, task_id: &str) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(task_id)
            .map(|s| s.get_all_output())
            .unwrap_or_default()
    }

    /// Get session status
    pub fn get_status(&self, task_id: &str) -> Option<SessionStatus> {
        let sessions = self.sessions.read().unwrap();
        sessions.get(task_id).map(|s| s.status.clone())
    }

    /// Get harness session ID for continuation
    pub fn get_session_id(&self, task_id: &str) -> Option<String> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(task_id)
            .filter(|s| !s.session_id.is_empty())
            .map(|s| s.session_id.clone())
    }

    /// List all active task IDs
    pub fn active_tasks(&self) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .iter()
            .filter(|(_, s)| s.is_active())
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Get all task IDs
    pub fn all_tasks(&self) -> Vec<String> {
        let sessions = self.sessions.read().unwrap();
        sessions.keys().cloned().collect()
    }

    /// Check if a session exists
    pub fn has_session(&self, task_id: &str) -> bool {
        let sessions = self.sessions.read().unwrap();
        sessions.contains_key(task_id)
    }

    /// Remove a session
    pub fn remove_session(&self, task_id: &str) -> Option<SessionStream> {
        let mut sessions = self.sessions.write().unwrap();
        sessions.remove(task_id)
    }

    /// Get session statistics
    pub fn session_stats(&self, task_id: &str) -> Option<(usize, usize)> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(task_id)
            .map(|s| (s.event_count(), s.line_count()))
    }

    /// Save session metadata for later continuation
    ///
    /// Persists the session ID, task ID, tag, and PID to a JSON file
    /// in the `.scud/headless/` directory for use by the `attach` command.
    pub fn save_session_metadata(&self, task_id: &str, project_root: &Path) -> Result<()> {
        let sessions = self.sessions.read().unwrap();
        let session = sessions
            .get(task_id)
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", task_id))?;

        let metadata_dir = project_root.join(".scud").join("headless");
        std::fs::create_dir_all(&metadata_dir)?;

        let metadata = serde_json::json!({
            "task_id": session.task_id,
            "session_id": session.session_id,
            "tag": session.tag,
            "pid": session.pid,
            "status": format!("{:?}", session.status),
            "started_at_ms": session.started_at.elapsed().as_millis() as u64,
        });

        let metadata_file = metadata_dir.join(format!("{}.json", task_id));
        std::fs::write(&metadata_file, serde_json::to_string_pretty(&metadata)?)?;

        Ok(())
    }

    /// Load session metadata from disk
    ///
    /// Returns the stored session_id if available for continuation.
    pub fn load_session_metadata(task_id: &str, project_root: &Path) -> Result<Option<String>> {
        let metadata_file = project_root
            .join(".scud")
            .join("headless")
            .join(format!("{}.json", task_id));

        if !metadata_file.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&metadata_file)?;
        let data: serde_json::Value = serde_json::from_str(&content)?;

        Ok(data
            .get("session_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_stream_new() {
        let stream = SessionStream::new("task-1", "phase-a");
        assert_eq!(stream.task_id, "task-1");
        assert_eq!(stream.tag, "phase-a");
        assert_eq!(stream.status, SessionStatus::Starting);
        assert!(stream.session_id.is_empty());
        assert!(stream.events.is_empty());
        assert!(stream.output_lines.is_empty());
    }

    #[test]
    fn test_push_text_delta_single_line() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::text_delta("Hello world"));

        // Text without newline stays in partial buffer
        assert_eq!(stream.output_lines.len(), 0);
        assert_eq!(stream.partial_line, "Hello world");
        assert_eq!(stream.events.len(), 1);
    }

    #[test]
    fn test_push_text_delta_with_newline() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::text_delta("Hello\nWorld\n"));

        assert_eq!(stream.output_lines.len(), 2);
        assert_eq!(stream.output_lines[0], "Hello");
        assert_eq!(stream.output_lines[1], "World");
        assert!(stream.partial_line.is_empty());
    }

    #[test]
    fn test_push_text_delta_incremental() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::text_delta("Hel"));
        stream.push_event(StreamEvent::text_delta("lo "));
        stream.push_event(StreamEvent::text_delta("world\n"));

        assert_eq!(stream.output_lines.len(), 1);
        assert_eq!(stream.output_lines[0], "Hello world");
    }

    #[test]
    fn test_push_tool_start() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::text_delta("Some text"));
        stream.push_event(StreamEvent::tool_start("Read", "tool-1", "src/main.rs"));

        // Tool start should flush partial line
        assert_eq!(stream.output_lines.len(), 2);
        assert_eq!(stream.output_lines[0], "Some text");
        assert_eq!(stream.output_lines[1], ">> Read src/main.rs");
    }

    #[test]
    fn test_push_tool_result() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::new(StreamEventKind::ToolResult {
            tool_name: "Read".to_string(),
            tool_id: "tool-1".to_string(),
            success: true,
        }));

        assert_eq!(stream.output_lines.len(), 1);
        assert_eq!(stream.output_lines[0], "<< Read ok");
    }

    #[test]
    fn test_push_tool_result_failed() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::new(StreamEventKind::ToolResult {
            tool_name: "Bash".to_string(),
            tool_id: "tool-2".to_string(),
            success: false,
        }));

        assert_eq!(stream.output_lines[0], "<< Bash failed");
    }

    #[test]
    fn test_session_assigned() {
        let mut stream = SessionStream::new("task-1", "test");
        assert_eq!(stream.status, SessionStatus::Starting);

        stream.push_event(StreamEvent::new(StreamEventKind::SessionAssigned {
            session_id: "sess-abc123".to_string(),
        }));

        assert_eq!(stream.session_id, "sess-abc123");
        assert_eq!(stream.status, SessionStatus::Running);
    }

    #[test]
    fn test_complete_success() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::complete(true));

        assert_eq!(stream.status, SessionStatus::Completed);
    }

    #[test]
    fn test_complete_failure() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::complete(false));

        assert_eq!(stream.status, SessionStatus::Failed);
    }

    #[test]
    fn test_error_event() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::error("Something went wrong"));

        assert_eq!(stream.status, SessionStatus::Failed);
        assert_eq!(stream.output_lines[0], "ERROR: Something went wrong");
    }

    #[test]
    fn test_tail() {
        let mut stream = SessionStream::new("task-1", "test");
        for i in 0..10 {
            stream.push_event(StreamEvent::text_delta(&format!("Line {}\n", i)));
        }

        let last3 = stream.tail(3);
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0], "Line 7");
        assert_eq!(last3[1], "Line 8");
        assert_eq!(last3[2], "Line 9");
    }

    #[test]
    fn test_tail_less_than_requested() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::text_delta("Only one\n"));

        let last10 = stream.tail(10);
        assert_eq!(last10.len(), 1);
        assert_eq!(last10[0], "Only one");
    }

    #[test]
    fn test_get_all_output_with_partial() {
        let mut stream = SessionStream::new("task-1", "test");
        stream.push_event(StreamEvent::text_delta("Complete line\n"));
        stream.push_event(StreamEvent::text_delta("Partial"));

        let output = stream.get_all_output();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0], "Complete line");
        assert_eq!(output[1], "Partial");
    }

    #[test]
    fn test_is_active() {
        let mut stream = SessionStream::new("task-1", "test");
        assert!(stream.is_active()); // Starting

        stream.status = SessionStatus::Running;
        assert!(stream.is_active());

        stream.status = SessionStatus::Completed;
        assert!(!stream.is_active());

        stream.status = SessionStatus::Failed;
        assert!(!stream.is_active());
    }

    #[test]
    fn test_event_timestamp() {
        let mut stream = SessionStream::new("task-1", "test");

        // Small sleep to ensure non-zero timestamp
        std::thread::sleep(std::time::Duration::from_millis(10));

        stream.push_event(StreamEvent::text_delta("Hello"));
        assert!(stream.events[0].timestamp_ms > 0);
    }

    // StreamStore tests

    #[test]
    fn test_store_create_session() {
        let store = StreamStore::new();
        let key = store.create_session("task-1", "phase-a");

        assert_eq!(key, "task-1");
        assert!(store.has_session("task-1"));
    }

    #[test]
    fn test_store_push_event() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        store.push_event("task-1", StreamEvent::text_delta("Hello\n"));

        let output = store.get_output("task-1", 100);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0], "Hello");
    }

    #[test]
    fn test_store_set_session_id() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        store.set_session_id("task-1", "sess-xyz");

        let session_id = store.get_session_id("task-1");
        assert_eq!(session_id, Some("sess-xyz".to_string()));
    }

    #[test]
    fn test_store_set_pid() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        store.set_pid("task-1", 12345);

        // Verify by checking stats or through save_session_metadata
        assert!(store.has_session("task-1"));
    }

    #[test]
    fn test_store_get_status() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");

        assert_eq!(store.get_status("task-1"), Some(SessionStatus::Starting));

        store.push_event("task-1", StreamEvent::complete(true));
        assert_eq!(store.get_status("task-1"), Some(SessionStatus::Completed));
    }

    #[test]
    fn test_store_active_tasks() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        store.create_session("task-2", "phase-a");
        store.push_event("task-2", StreamEvent::complete(true));

        let active = store.active_tasks();
        assert_eq!(active.len(), 1);
        assert!(active.contains(&"task-1".to_string()));
    }

    #[test]
    fn test_store_all_tasks() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        store.create_session("task-2", "phase-b");

        let all = store.all_tasks();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_store_remove_session() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        assert!(store.has_session("task-1"));

        let removed = store.remove_session("task-1");
        assert!(removed.is_some());
        assert!(!store.has_session("task-1"));
    }

    #[test]
    fn test_store_session_stats() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        store.push_event("task-1", StreamEvent::text_delta("Line 1\n"));
        store.push_event("task-1", StreamEvent::text_delta("Line 2\n"));

        let stats = store.session_stats("task-1");
        assert!(stats.is_some());
        let (events, lines) = stats.unwrap();
        assert_eq!(events, 2);
        assert_eq!(lines, 2);
    }

    #[test]
    fn test_store_nonexistent_session() {
        let store = StreamStore::new();

        assert_eq!(store.get_output("nonexistent", 100), Vec::<String>::new());
        assert_eq!(store.get_status("nonexistent"), None);
        assert_eq!(store.get_session_id("nonexistent"), None);
    }

    #[test]
    fn test_store_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let store = Arc::new(StreamStore::new());
        store.create_session("task-1", "phase-a");

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let store = Arc::clone(&store);
                thread::spawn(move || {
                    for j in 0..100 {
                        store.push_event(
                            "task-1",
                            StreamEvent::text_delta(&format!("Thread {} line {}\n", i, j)),
                        );
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = store.session_stats("task-1").unwrap();
        assert_eq!(stats.0, 1000); // 10 threads * 100 events
        assert_eq!(stats.1, 1000); // 10 threads * 100 lines
    }

    #[test]
    fn test_memory_limit_output_lines() {
        let mut stream = SessionStream::new("task-1", "test");

        // Push more than MAX_OUTPUT_LINES
        for i in 0..MAX_OUTPUT_LINES + 1000 {
            stream.push_event(StreamEvent::text_delta(&format!("Line {}\n", i)));
        }

        // Should have trimmed to within limits
        assert!(stream.output_lines.len() <= MAX_OUTPUT_LINES);
    }

    #[test]
    fn test_memory_limit_events() {
        let mut stream = SessionStream::new("task-1", "test");

        // Push more than MAX_EVENTS
        for i in 0..MAX_EVENTS + 1000 {
            stream.push_event(StreamEvent::text_delta(&format!("{}", i)));
        }

        // Should have trimmed to within limits
        assert!(stream.events.len() <= MAX_EVENTS);
    }

    #[test]
    fn test_save_and_load_session_metadata() {
        let temp_dir = std::env::temp_dir().join(format!("scud_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        store.set_session_id("task-1", "sess-abc123");
        store.set_pid("task-1", 12345);

        // Save metadata
        store.save_session_metadata("task-1", &temp_dir).unwrap();

        // Verify file exists
        let metadata_file = temp_dir.join(".scud").join("headless").join("task-1.json");
        assert!(metadata_file.exists());

        // Load metadata
        let loaded = StreamStore::load_session_metadata("task-1", &temp_dir).unwrap();
        assert_eq!(loaded, Some("sess-abc123".to_string()));

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_load_nonexistent_metadata() {
        let temp_dir = std::env::temp_dir().join(format!("scud_test_ne_{}", std::process::id()));
        let loaded = StreamStore::load_session_metadata("nonexistent", &temp_dir).unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn test_get_session_id_empty_string() {
        let store = StreamStore::new();
        store.create_session("task-1", "phase-a");
        // Session ID is empty by default

        // Should return None for empty session ID
        let session_id = store.get_session_id("task-1");
        assert_eq!(session_id, None);
    }
}
