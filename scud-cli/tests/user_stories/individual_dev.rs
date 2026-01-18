//! Individual Developer Workflow Tests (US-1 to US-5)
//!
//! Tests for core developer workflow commands:
//! - US-1: Morning Session Orientation (warmup)
//! - US-2: Task Discovery and Selection (next, show)
//! - US-3: Cross-Phase Task Discovery (next --all-tags)
//! - US-4: Task-Aware Git Commits (commit)
//! - US-5: Progress Logging (log)

use crate::e2e::fixtures::TestProject;

// US-1: Morning Session Orientation

#[test]
fn test_us1_warmup_shows_active_tasks() {
    let project = TestProject::simple_project();
    let phase = project.storage.load_active_group().unwrap();

    // Warmup should identify tasks - verify the data is available
    let stats = phase.get_stats();
    assert!(stats.total > 0, "Project should have tasks");
}

#[test]
fn test_us1_warmup_shows_next_task() {
    let project = TestProject::simple_project();
    let phase = project.storage.load_active_group().unwrap();

    // Should find next available task
    let next_task = phase.find_next_task();
    assert!(next_task.is_some(), "Should have a next task available");

    // First task in chain should be the next one (no dependencies)
    let next = next_task.unwrap();
    assert_eq!(next.id, "1", "Task 1 should be next (no dependencies)");
}

// US-2: Task Discovery and Selection

#[test]
fn test_us2_next_finds_ready_task() {
    let project = TestProject::simple_project();
    let phase = project.storage.load_active_group().unwrap();

    // Task 1 has no dependencies, should be ready
    let next = phase.find_next_task().unwrap();
    assert_eq!(next.id, "1");
    assert!(next.dependencies.is_empty());
}

#[test]
fn test_us2_next_respects_dependencies() {
    let project = TestProject::simple_project();
    let mut phase = project.storage.load_active_group().unwrap();

    // Mark task 1 as done
    phase
        .get_task_mut("1")
        .unwrap()
        .set_status(scud::models::TaskStatus::Done);
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Reload and find next
    let phase = project.storage.load_active_group().unwrap();
    let next = phase.find_next_task().unwrap();

    // Now task 2 should be ready
    assert_eq!(next.id, "2");
}

#[test]
fn test_us2_show_displays_task_details() {
    let project = TestProject::complexity_project();
    let phase = project.storage.load_active_group().unwrap();

    // Get task with complexity
    let task = phase.get_task("2").unwrap();

    // Verify task has details
    assert!(!task.title.is_empty());
    assert!(!task.description.is_empty());
    assert!(task.complexity > 0);
}

// US-3: Cross-Phase Task Discovery

#[test]
fn test_us3_multi_phase_project_structure() {
    let project = TestProject::multi_phase_project();
    let tasks = project.storage.load_tasks().unwrap();

    // Should have multiple phases
    assert!(tasks.len() >= 2);
    assert!(tasks.contains_key("phase1"));
    assert!(tasks.contains_key("phase2"));
}

#[test]
fn test_us3_cross_phase_dependency_exists() {
    let project = TestProject::multi_phase_project();
    let tasks = project.storage.load_tasks().unwrap();

    // Phase 2 task should depend on phase 1
    let phase2 = tasks.get("phase2").unwrap();
    let task = phase2.get_task("1").unwrap();
    assert!(task.dependencies.iter().any(|d| d.starts_with("phase1:")));
}

// US-4: Task-Aware Git Commits

#[test]
fn test_us4_current_task_file() {
    let project = TestProject::simple_project();

    // Set current task
    project.set_current_task("1.2").unwrap();

    // Verify file exists
    let current_task_file = project.path.join(".scud").join("current-task");
    assert!(current_task_file.exists());

    // Verify content
    let content = std::fs::read_to_string(current_task_file).unwrap();
    assert_eq!(content, "1.2");
}

#[test]
fn test_us4_git_repo_can_be_initialized() {
    let project = TestProject::simple_project();
    project.init_git().unwrap();

    // Verify git directory exists
    assert!(project.path.join(".git").exists());
}

// US-5: Progress Logging

#[test]
fn test_us5_task_can_have_log_entries() {
    let project = TestProject::simple_project();
    let mut phase = project.storage.load_active_group().unwrap();

    // Get task and add to details (log entries go in details)
    let task = phase.get_task_mut("1").unwrap();
    let original_details = task.details.clone().unwrap_or_default();
    task.details = Some(format!(
        "{}\n\n## Log\n- Started implementation",
        original_details
    ));

    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Reload and verify
    let phase = project.storage.load_active_group().unwrap();
    let task = phase.get_task("1").unwrap();
    assert!(task
        .details
        .as_ref()
        .unwrap()
        .contains("Started implementation"));
}

#[test]
fn test_us5_multiple_log_entries() {
    let project = TestProject::simple_project();
    let mut phase = project.storage.load_active_group().unwrap();

    // Add multiple log entries
    let task = phase.get_task_mut("1").unwrap();
    task.details = Some("## Log\n- Entry 1\n- Entry 2\n- Entry 3".to_string());
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Verify all entries persisted
    let phase = project.storage.load_active_group().unwrap();
    let task = phase.get_task("1").unwrap();
    let details = task.details.as_ref().unwrap();
    assert!(details.contains("Entry 1"));
    assert!(details.contains("Entry 2"));
    assert!(details.contains("Entry 3"));
}
