//! Mock terminal manager for spawn command testing
//!
//! Provides `MockTerminalManager` for testing spawn commands without
//! actually creating terminal sessions. When the `real-terminal` feature
//! is enabled, tests can optionally use real tmux/kitty sessions.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Represents a terminal type for spawning sessions
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalType {
    Tmux,
    Kitty,
    ITerm2,
    VSCode,
    WezTerm,
    Mock,
}

impl std::fmt::Display for TerminalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminalType::Tmux => write!(f, "tmux"),
            TerminalType::Kitty => write!(f, "kitty"),
            TerminalType::ITerm2 => write!(f, "iterm2"),
            TerminalType::VSCode => write!(f, "vscode"),
            TerminalType::WezTerm => write!(f, "wezterm"),
            TerminalType::Mock => write!(f, "mock"),
        }
    }
}

/// Status of a spawned session
#[derive(Debug, Clone, PartialEq)]
pub enum SessionStatus {
    Running,
    Completed,
    Failed(String),
    Terminated,
}

/// Information about a spawned session
#[derive(Debug, Clone)]
pub struct SpawnedSession {
    /// Unique session identifier
    pub id: String,
    /// Task ID this session is working on
    pub task_id: String,
    /// Command being executed
    pub command: String,
    /// Terminal type used
    pub terminal_type: TerminalType,
    /// Current status
    pub status: SessionStatus,
    /// Working directory
    pub working_dir: String,
    /// When the session was created
    pub created_at: std::time::Instant,
    /// Optional output captured (for mock)
    pub output: Option<String>,
}

/// Mock terminal manager that records spawn requests without creating real sessions
#[derive(Clone)]
pub struct MockTerminalManager {
    /// Active sessions
    sessions: Arc<Mutex<HashMap<String, SpawnedSession>>>,
    /// Counter for generating session IDs
    next_id: Arc<Mutex<u32>>,
    /// Configured behavior for spawns
    spawn_behavior: Arc<Mutex<SpawnBehavior>>,
}

/// Configurable behavior for mock spawns
#[derive(Clone)]
pub enum SpawnBehavior {
    /// Always succeed
    AlwaysSucceed,
    /// Always fail with error
    AlwaysFail(String),
    /// Fail for specific task IDs
    FailForTasks(Vec<String>),
    /// Custom behavior per task
    Custom(Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>),
}

impl Default for SpawnBehavior {
    fn default() -> Self {
        SpawnBehavior::AlwaysSucceed
    }
}

impl MockTerminalManager {
    /// Create a new mock terminal manager
    pub fn new() -> Self {
        MockTerminalManager {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
            spawn_behavior: Arc::new(Mutex::new(SpawnBehavior::default())),
        }
    }

    /// Configure spawn behavior
    pub fn with_behavior(self, behavior: SpawnBehavior) -> Self {
        *self.spawn_behavior.lock().unwrap() = behavior;
        self
    }

    /// Configure to fail for specific task IDs
    pub fn fail_for_tasks(self, task_ids: Vec<String>) -> Self {
        self.with_behavior(SpawnBehavior::FailForTasks(task_ids))
    }

    /// Spawn a new session (mock implementation)
    pub fn spawn(
        &self,
        task_id: &str,
        command: &str,
        working_dir: &str,
        terminal: TerminalType,
    ) -> Result<String, String> {
        // Check configured behavior
        let behavior = self.spawn_behavior.lock().unwrap().clone();
        match behavior {
            SpawnBehavior::AlwaysSucceed => {}
            SpawnBehavior::AlwaysFail(msg) => return Err(msg),
            SpawnBehavior::FailForTasks(tasks) => {
                if tasks.contains(&task_id.to_string()) {
                    return Err(format!("Configured to fail for task {}", task_id));
                }
            }
            SpawnBehavior::Custom(f) => {
                f(task_id)?;
            }
        }

        // Generate session ID
        let mut id_counter = self.next_id.lock().unwrap();
        let session_id = format!("session-{}", *id_counter);
        *id_counter += 1;

        // Create session record
        let session = SpawnedSession {
            id: session_id.clone(),
            task_id: task_id.to_string(),
            command: command.to_string(),
            terminal_type: terminal,
            status: SessionStatus::Running,
            working_dir: working_dir.to_string(),
            created_at: std::time::Instant::now(),
            output: None,
        };

        self.sessions.lock().unwrap().insert(session_id.clone(), session);
        Ok(session_id)
    }

    /// Get all active sessions
    pub fn sessions(&self) -> Vec<SpawnedSession> {
        self.sessions.lock().unwrap().values().cloned().collect()
    }

    /// Get sessions for a specific task
    pub fn sessions_for_task(&self, task_id: &str) -> Vec<SpawnedSession> {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.task_id == task_id)
            .cloned()
            .collect()
    }

    /// Get a specific session by ID
    pub fn get_session(&self, session_id: &str) -> Option<SpawnedSession> {
        self.sessions.lock().unwrap().get(session_id).cloned()
    }

    /// Update session status
    pub fn update_status(&self, session_id: &str, status: SessionStatus) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.status = status;
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    /// Mark a session as completed with output
    pub fn complete_session(&self, session_id: &str, output: Option<String>) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(session_id) {
            session.status = SessionStatus::Completed;
            session.output = output;
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    /// Terminate a session
    pub fn terminate(&self, session_id: &str) -> Result<(), String> {
        self.update_status(session_id, SessionStatus::Terminated)
    }

    /// Get count of running sessions
    pub fn running_count(&self) -> usize {
        self.sessions
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.status == SessionStatus::Running)
            .count()
    }

    /// Clear all sessions
    pub fn clear(&self) {
        self.sessions.lock().unwrap().clear();
        *self.next_id.lock().unwrap() = 1;
    }

    /// Check if any session was spawned
    pub fn any_spawned(&self) -> bool {
        !self.sessions.lock().unwrap().is_empty()
    }

    /// Get total spawn count
    pub fn spawn_count(&self) -> usize {
        self.sessions.lock().unwrap().len()
    }
}

impl Default for MockTerminalManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper trait for terminal detection (used in real implementation)
pub trait TerminalDetector {
    fn detect_available() -> Vec<TerminalType>;
    fn is_available(terminal: &TerminalType) -> bool;
}

/// Mock terminal detector that reports all terminals as available
pub struct MockTerminalDetector;

impl TerminalDetector for MockTerminalDetector {
    fn detect_available() -> Vec<TerminalType> {
        vec![
            TerminalType::Tmux,
            TerminalType::Kitty,
            TerminalType::Mock,
        ]
    }

    fn is_available(_terminal: &TerminalType) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_creates_session() {
        let manager = MockTerminalManager::new();

        let session_id = manager
            .spawn("task-1", "claude code", "/project", TerminalType::Tmux)
            .unwrap();

        assert!(session_id.starts_with("session-"));
        assert!(manager.any_spawned());
        assert_eq!(manager.spawn_count(), 1);
    }

    #[test]
    fn test_spawn_multiple_sessions() {
        let manager = MockTerminalManager::new();

        manager.spawn("task-1", "cmd1", "/p1", TerminalType::Tmux).unwrap();
        manager.spawn("task-2", "cmd2", "/p2", TerminalType::Kitty).unwrap();
        manager.spawn("task-3", "cmd3", "/p3", TerminalType::Mock).unwrap();

        assert_eq!(manager.spawn_count(), 3);

        let sessions = manager.sessions();
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn test_sessions_for_task() {
        let manager = MockTerminalManager::new();

        manager.spawn("task-1", "cmd1", "/p", TerminalType::Tmux).unwrap();
        manager.spawn("task-1", "cmd2", "/p", TerminalType::Tmux).unwrap();
        manager.spawn("task-2", "cmd3", "/p", TerminalType::Tmux).unwrap();

        let task1_sessions = manager.sessions_for_task("task-1");
        assert_eq!(task1_sessions.len(), 2);

        let task2_sessions = manager.sessions_for_task("task-2");
        assert_eq!(task2_sessions.len(), 1);
    }

    #[test]
    fn test_session_status_updates() {
        let manager = MockTerminalManager::new();

        let session_id = manager
            .spawn("task-1", "cmd", "/p", TerminalType::Tmux)
            .unwrap();

        // Initially running
        let session = manager.get_session(&session_id).unwrap();
        assert_eq!(session.status, SessionStatus::Running);

        // Update to completed
        manager.complete_session(&session_id, Some("Done!".to_string())).unwrap();
        let session = manager.get_session(&session_id).unwrap();
        assert_eq!(session.status, SessionStatus::Completed);
        assert_eq!(session.output, Some("Done!".to_string()));
    }

    #[test]
    fn test_terminate_session() {
        let manager = MockTerminalManager::new();

        let session_id = manager
            .spawn("task-1", "cmd", "/p", TerminalType::Tmux)
            .unwrap();

        manager.terminate(&session_id).unwrap();

        let session = manager.get_session(&session_id).unwrap();
        assert_eq!(session.status, SessionStatus::Terminated);
    }

    #[test]
    fn test_running_count() {
        let manager = MockTerminalManager::new();

        let s1 = manager.spawn("task-1", "cmd", "/p", TerminalType::Tmux).unwrap();
        let s2 = manager.spawn("task-2", "cmd", "/p", TerminalType::Tmux).unwrap();
        manager.spawn("task-3", "cmd", "/p", TerminalType::Tmux).unwrap();

        assert_eq!(manager.running_count(), 3);

        manager.complete_session(&s1, None).unwrap();
        assert_eq!(manager.running_count(), 2);

        manager.terminate(&s2).unwrap();
        assert_eq!(manager.running_count(), 1);
    }

    #[test]
    fn test_always_fail_behavior() {
        let manager = MockTerminalManager::new()
            .with_behavior(SpawnBehavior::AlwaysFail("No terminal available".to_string()));

        let result = manager.spawn("task-1", "cmd", "/p", TerminalType::Tmux);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No terminal available");
    }

    #[test]
    fn test_fail_for_specific_tasks() {
        let manager = MockTerminalManager::new()
            .fail_for_tasks(vec!["task-2".to_string(), "task-4".to_string()]);

        // These should succeed
        assert!(manager.spawn("task-1", "cmd", "/p", TerminalType::Tmux).is_ok());
        assert!(manager.spawn("task-3", "cmd", "/p", TerminalType::Tmux).is_ok());

        // These should fail
        assert!(manager.spawn("task-2", "cmd", "/p", TerminalType::Tmux).is_err());
        assert!(manager.spawn("task-4", "cmd", "/p", TerminalType::Tmux).is_err());
    }

    #[test]
    fn test_clear_sessions() {
        let manager = MockTerminalManager::new();

        manager.spawn("task-1", "cmd", "/p", TerminalType::Tmux).unwrap();
        manager.spawn("task-2", "cmd", "/p", TerminalType::Tmux).unwrap();

        assert_eq!(manager.spawn_count(), 2);

        manager.clear();

        assert_eq!(manager.spawn_count(), 0);
        assert!(!manager.any_spawned());
    }

    #[test]
    fn test_session_records_details() {
        let manager = MockTerminalManager::new();

        let session_id = manager
            .spawn("my-task", "claude code --prompt 'fix bug'", "/home/user/project", TerminalType::Kitty)
            .unwrap();

        let session = manager.get_session(&session_id).unwrap();
        assert_eq!(session.task_id, "my-task");
        assert_eq!(session.command, "claude code --prompt 'fix bug'");
        assert_eq!(session.working_dir, "/home/user/project");
        assert_eq!(session.terminal_type, TerminalType::Kitty);
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;
        use std::sync::Arc;

        let manager = Arc::new(MockTerminalManager::new());
        let mut handles = vec![];

        for i in 0..10 {
            let m = Arc::clone(&manager);
            let handle = thread::spawn(move || {
                m.spawn(&format!("task-{}", i), "cmd", "/p", TerminalType::Mock).unwrap();
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(manager.spawn_count(), 10);
    }

    #[test]
    fn test_terminal_type_display() {
        assert_eq!(format!("{}", TerminalType::Tmux), "tmux");
        assert_eq!(format!("{}", TerminalType::Kitty), "kitty");
        assert_eq!(format!("{}", TerminalType::Mock), "mock");
    }

    #[test]
    fn test_get_nonexistent_session() {
        let manager = MockTerminalManager::new();
        assert!(manager.get_session("nonexistent").is_none());
    }

    #[test]
    fn test_update_nonexistent_session() {
        let manager = MockTerminalManager::new();
        let result = manager.update_status("nonexistent", SessionStatus::Completed);
        assert!(result.is_err());
    }
}
