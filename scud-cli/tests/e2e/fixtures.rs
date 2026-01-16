//! Test fixtures for SCUD E2E tests
//!
//! Provides `TestProject` for creating temporary SCUD project structures.

use scud::models::{Phase, Task, TaskStatus};
use scud::storage::Storage;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// A test project with a temporary directory and SCUD storage
pub struct TestProject {
    /// The temporary directory - must be kept alive for the duration of the test
    pub dir: TempDir,
    /// Path to the project root
    pub path: PathBuf,
    /// Storage instance for the project
    pub storage: Storage,
}

impl TestProject {
    /// Create a minimal SCUD project structure
    pub fn new() -> Self {
        let dir = TempDir::new().expect("Failed to create temp dir");
        let path = dir.path().to_path_buf();
        let storage = Storage::new(Some(path.clone()));
        storage.initialize().expect("Failed to initialize storage");

        TestProject { dir, path, storage }
    }

    /// Create a project with custom SCG tasks
    pub fn with_tasks(scg_content: &str) -> Self {
        let project = Self::new();
        let tasks_file = project.storage.tasks_file();

        // Ensure parent directory exists
        if let Some(parent) = tasks_file.parent() {
            fs::create_dir_all(parent).expect("Failed to create tasks directory");
        }

        fs::write(&tasks_file, scg_content).expect("Failed to write tasks file");
        project
    }

    /// Create a simple project with 3 tasks in a linear dependency chain
    /// Task 1 (Setup) -> Task 2 (Build) -> Task 3 (Test)
    pub fn simple_project() -> Self {
        let project = Self::new();

        let mut phase = Phase::new("test-phase".to_string());

        let task1 = Task::new("1".to_string(), "Setup project".to_string(), "Initial setup".to_string());

        let mut task2 = Task::new("2".to_string(), "Build feature".to_string(), "Build the main feature".to_string());
        task2.dependencies = vec!["1".to_string()];

        let mut task3 = Task::new("3".to_string(), "Write tests".to_string(), "Add test coverage".to_string());
        task3.dependencies = vec!["2".to_string()];

        phase.add_task(task1);
        phase.add_task(task2);
        phase.add_task(task3);

        let mut tasks = HashMap::new();
        tasks.insert("test-phase".to_string(), phase);
        project.storage.save_tasks(&tasks).expect("Failed to save tasks");
        project.storage.set_active_group("test-phase").expect("Failed to set active group");

        project
    }

    /// Create a project with parallel tasks (2 independent + 1 dependent on both)
    /// Task 1, Task 2 (parallel) -> Task 3 (depends on both)
    pub fn parallel_project() -> Self {
        let project = Self::new();

        let mut phase = Phase::new("parallel-phase".to_string());

        let task1 = Task::new("1".to_string(), "Feature A".to_string(), "Build feature A".to_string());
        let task2 = Task::new("2".to_string(), "Feature B".to_string(), "Build feature B".to_string());

        let mut task3 = Task::new("3".to_string(), "Integration".to_string(), "Integrate features".to_string());
        task3.dependencies = vec!["1".to_string(), "2".to_string()];

        phase.add_task(task1);
        phase.add_task(task2);
        phase.add_task(task3);

        let mut tasks = HashMap::new();
        tasks.insert("parallel-phase".to_string(), phase);
        project.storage.save_tasks(&tasks).expect("Failed to save tasks");
        project.storage.set_active_group("parallel-phase").expect("Failed to set active group");

        project
    }

    /// Create a project with multiple phases for cross-phase dependency testing
    pub fn multi_phase_project() -> Self {
        let project = Self::new();

        // Phase 1: Foundation
        let mut phase1 = Phase::new("phase1".to_string());
        let mut task1_1 = Task::new("1".to_string(), "Database setup".to_string(), "Set up database schema".to_string());
        task1_1.status = TaskStatus::Pending;
        phase1.add_task(task1_1);

        // Phase 2: API (depends on phase 1)
        let mut phase2 = Phase::new("phase2".to_string());
        let mut task2_1 = Task::new("1".to_string(), "API endpoints".to_string(), "Create REST API".to_string());
        task2_1.dependencies = vec!["phase1:1".to_string()]; // Cross-phase dependency
        task2_1.status = TaskStatus::Pending;
        phase2.add_task(task2_1);

        let mut tasks = HashMap::new();
        tasks.insert("phase1".to_string(), phase1);
        tasks.insert("phase2".to_string(), phase2);
        project.storage.save_tasks(&tasks).expect("Failed to save tasks");
        project.storage.set_active_group("phase1").expect("Failed to set active group");

        project
    }

    /// Create a project with complex wave structure for wave computation testing
    /// Wave 1: Tasks 1, 2 (no deps)
    /// Wave 2: Tasks 3, 4 (depend on wave 1)
    /// Wave 3: Task 5 (depends on wave 2)
    pub fn wave_project() -> Self {
        let project = Self::new();

        let mut phase = Phase::new("wave-phase".to_string());

        // Wave 1 - no dependencies
        let task1 = Task::new("1".to_string(), "Research".to_string(), "Research phase".to_string());
        let task2 = Task::new("2".to_string(), "Design".to_string(), "Design phase".to_string());

        // Wave 2 - depends on wave 1
        let mut task3 = Task::new("3".to_string(), "Implement A".to_string(), "Implementation A".to_string());
        task3.dependencies = vec!["1".to_string()];

        let mut task4 = Task::new("4".to_string(), "Implement B".to_string(), "Implementation B".to_string());
        task4.dependencies = vec!["2".to_string()];

        // Wave 3 - depends on wave 2
        let mut task5 = Task::new("5".to_string(), "Integrate".to_string(), "Final integration".to_string());
        task5.dependencies = vec!["3".to_string(), "4".to_string()];

        phase.add_task(task1);
        phase.add_task(task2);
        phase.add_task(task3);
        phase.add_task(task4);
        phase.add_task(task5);

        let mut tasks = HashMap::new();
        tasks.insert("wave-phase".to_string(), phase);
        project.storage.save_tasks(&tasks).expect("Failed to save tasks");
        project.storage.set_active_group("wave-phase").expect("Failed to set active group");

        project
    }

    /// Create a project with various task statuses for statistics testing
    pub fn stats_project() -> Self {
        let project = Self::new();

        let mut phase = Phase::new("stats-phase".to_string());

        // 3 pending
        let mut t1 = Task::new("1".to_string(), "Pending 1".to_string(), "".to_string());
        t1.status = TaskStatus::Pending;
        let mut t2 = Task::new("2".to_string(), "Pending 2".to_string(), "".to_string());
        t2.status = TaskStatus::Pending;
        let mut t3 = Task::new("3".to_string(), "Pending 3".to_string(), "".to_string());
        t3.status = TaskStatus::Pending;

        // 2 done
        let mut t4 = Task::new("4".to_string(), "Done 1".to_string(), "".to_string());
        t4.status = TaskStatus::Done;
        let mut t5 = Task::new("5".to_string(), "Done 2".to_string(), "".to_string());
        t5.status = TaskStatus::Done;

        // 1 in-progress
        let mut t6 = Task::new("6".to_string(), "In Progress 1".to_string(), "".to_string());
        t6.status = TaskStatus::InProgress;

        phase.add_task(t1);
        phase.add_task(t2);
        phase.add_task(t3);
        phase.add_task(t4);
        phase.add_task(t5);
        phase.add_task(t6);

        let mut tasks = HashMap::new();
        tasks.insert("stats-phase".to_string(), phase);
        project.storage.save_tasks(&tasks).expect("Failed to save tasks");
        project.storage.set_active_group("stats-phase").expect("Failed to set active group");

        project
    }

    /// Create a project with tasks that have assignments
    pub fn assigned_project() -> Self {
        let project = Self::new();

        let mut phase = Phase::new("assigned-phase".to_string());

        let mut t1 = Task::new("1".to_string(), "Task for Alice".to_string(), "".to_string());
        t1.assigned_to = Some("Alice".to_string());
        t1.status = TaskStatus::InProgress;

        let mut t2 = Task::new("2".to_string(), "Task for Bob".to_string(), "".to_string());
        t2.assigned_to = Some("Bob".to_string());
        t2.status = TaskStatus::InProgress;

        let t3 = Task::new("3".to_string(), "Unassigned task".to_string(), "".to_string());

        phase.add_task(t1);
        phase.add_task(t2);
        phase.add_task(t3);

        let mut tasks = HashMap::new();
        tasks.insert("assigned-phase".to_string(), phase);
        project.storage.save_tasks(&tasks).expect("Failed to save tasks");
        project.storage.set_active_group("assigned-phase").expect("Failed to set active group");

        project
    }

    /// Create a project with tasks that have complexity scores
    pub fn complexity_project() -> Self {
        let project = Self::new();

        let mut phase = Phase::new("complexity-phase".to_string());

        let mut t1 = Task::new("1".to_string(), "Simple task".to_string(), "A simple task".to_string());
        t1.complexity = 3;

        let mut t2 = Task::new("2".to_string(), "Medium task".to_string(), "A medium complexity task".to_string());
        t2.complexity = 8;

        let mut t3 = Task::new("3".to_string(), "Complex task".to_string(), "A highly complex task".to_string());
        t3.complexity = 13;

        phase.add_task(t1);
        phase.add_task(t2);
        phase.add_task(t3);

        let mut tasks = HashMap::new();
        tasks.insert("complexity-phase".to_string(), phase);
        project.storage.save_tasks(&tasks).expect("Failed to save tasks");
        project.storage.set_active_group("complexity-phase").expect("Failed to set active group");

        project
    }

    /// Get the active tag from the project
    pub fn tag(&self) -> String {
        self.storage
            .get_active_group()
            .expect("Failed to get active group")
            .expect("No active group set")
    }

    /// Initialize a git repository in the project for commit tests
    pub fn init_git(&self) -> std::io::Result<()> {
        use std::process::Command;

        Command::new("git")
            .args(["init"])
            .current_dir(&self.path)
            .output()?;

        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .current_dir(&self.path)
            .output()?;

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&self.path)
            .output()?;

        Ok(())
    }

    /// Write a file to the project directory
    pub fn write_file(&self, relative_path: &str, content: &str) -> std::io::Result<()> {
        let full_path = self.path.join(relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full_path, content)
    }

    /// Set the current task file (for commit prefixing)
    pub fn set_current_task(&self, task_id: &str) -> std::io::Result<()> {
        let current_task_file = self.path.join(".scud").join("current-task");
        fs::write(current_task_file, task_id)
    }
}

impl Default for TestProject {
    fn default() -> Self {
        Self::new()
    }
}

/// Sample PRD content for AI parsing tests
pub const SAMPLE_PRD: &str = r#"# Product Requirements Document

## Overview
Build a user authentication system.

## Features

### User Registration
- Allow users to register with email and password
- Validate email format
- Enforce password strength requirements

### Login System
- Implement login with email/password
- Add session management
- Support "remember me" functionality

### Password Reset
- Email-based password reset flow
- Secure token generation
- Token expiration after 24 hours
"#;

/// Sample SCG content for parsing tests
pub const SAMPLE_SCG: &str = r#"# SCUD Graph v1

@meta { name: "test-phase", id_format: "sequential" }

@nodes
1 P Setup project | c:3 p:high
2 P Build feature | c:5 p:high
3 P Write tests | c:3 p:medium

@edges
1 -> 2
2 -> 3

@details
1 | Initial project setup with dependencies
2 | Implement the main feature logic
3 | Add comprehensive test coverage
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_creates_scud_dir() {
        let project = TestProject::new();
        assert!(project.path.join(".scud").exists());
        assert!(project.path.join(".scud").join("tasks").exists());
    }

    #[test]
    fn test_simple_project_has_three_tasks() {
        let project = TestProject::simple_project();
        let phase = project.storage.load_active_group().unwrap();
        assert_eq!(phase.tasks.len(), 3);
    }

    #[test]
    fn test_parallel_project_structure() {
        let project = TestProject::parallel_project();
        let phase = project.storage.load_active_group().unwrap();

        // Task 3 should depend on both 1 and 2
        let task3 = phase.get_task("3").unwrap();
        assert!(task3.dependencies.contains(&"1".to_string()));
        assert!(task3.dependencies.contains(&"2".to_string()));
    }

    #[test]
    fn test_wave_project_structure() {
        let project = TestProject::wave_project();
        let phase = project.storage.load_active_group().unwrap();
        assert_eq!(phase.tasks.len(), 5);

        // Task 5 should depend on 3 and 4
        let task5 = phase.get_task("5").unwrap();
        assert_eq!(task5.dependencies.len(), 2);
    }

    #[test]
    fn test_stats_project_status_counts() {
        let project = TestProject::stats_project();
        let phase = project.storage.load_active_group().unwrap();
        let stats = phase.get_stats();

        assert_eq!(stats.pending, 3);
        assert_eq!(stats.done, 2);
        assert_eq!(stats.in_progress, 1);
        assert_eq!(stats.total, 6);
    }

    #[test]
    fn test_git_init() {
        let project = TestProject::new();
        project.init_git().unwrap();
        assert!(project.path.join(".git").exists());
    }

    #[test]
    fn test_write_file() {
        let project = TestProject::new();
        project.write_file("src/main.rs", "fn main() {}").unwrap();
        assert!(project.path.join("src").join("main.rs").exists());
    }

    #[test]
    fn test_multi_phase_project() {
        let project = TestProject::multi_phase_project();
        let tasks = project.storage.load_tasks().unwrap();

        assert!(tasks.contains_key("phase1"));
        assert!(tasks.contains_key("phase2"));

        // Check cross-phase dependency
        let phase2 = tasks.get("phase2").unwrap();
        let task = phase2.get_task("1").unwrap();
        assert!(task.dependencies.contains(&"phase1:1".to_string()));
    }
}
