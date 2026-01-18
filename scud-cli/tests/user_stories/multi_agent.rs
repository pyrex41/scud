//! Multi-Agent Spawn Tests (US-15 to US-17)
//!
//! Tests for parallel agent spawning features:
//! - US-15: Parallel Agent Spawning (spawn)
//! - US-16: Spawn Session Monitoring (monitor, sessions)
//! - US-17: Automatic Task Claiming (spawn --claim)
//!
//! These tests use MockTerminalManager by default. Run with `--features real-terminal`
//! to test with actual tmux/kitty sessions.

use crate::e2e::fixtures::TestProject;
use crate::e2e::mock_terminal::{MockTerminalManager, SessionStatus, SpawnBehavior, TerminalType};
use scud::models::TaskStatus;

// US-15: Parallel Agent Spawning

#[test]
fn test_us15_spawn_creates_sessions_for_ready_tasks() {
    let project = TestProject::parallel_project();
    let phase = project.storage.load_active_group().unwrap();
    let manager = MockTerminalManager::new();

    // Get actionable tasks (tasks with no unmet dependencies)
    let actionable = phase.get_actionable_tasks();

    // Spawn sessions for each actionable task
    for task in &actionable {
        manager
            .spawn(
                &task.id,
                &format!("claude code --task {}", task.id),
                project.path.to_str().unwrap(),
                TerminalType::Tmux,
            )
            .unwrap();
    }

    // parallel_project has tasks 1 and 2 with no deps, task 3 depends on both
    // get_actionable_tasks returns pending tasks with deps met
    // Should have at least 1 session (tasks without dependencies)
    assert!(
        manager.spawn_count() >= 1,
        "Should spawn at least one session for ready tasks"
    );
    assert!(
        manager.spawn_count() <= 3,
        "Should not spawn more than total tasks"
    );
}

#[test]
fn test_us15_spawn_respects_limit() {
    let project = TestProject::wave_project();
    let phase = project.storage.load_active_group().unwrap();
    let manager = MockTerminalManager::new();

    // Get actionable tasks (1 and 2 are both ready)
    let actionable = phase.get_actionable_tasks();
    let limit = 1;

    // Only spawn up to limit
    for task in actionable.iter().take(limit) {
        manager
            .spawn(
                &task.id,
                &format!("claude code --task {}", task.id),
                project.path.to_str().unwrap(),
                TerminalType::Tmux,
            )
            .unwrap();
    }

    assert_eq!(manager.spawn_count(), 1);
}

#[test]
fn test_us15_spawn_different_terminal_types() {
    let project = TestProject::parallel_project();
    let manager = MockTerminalManager::new();

    manager
        .spawn(
            "1",
            "cmd",
            project.path.to_str().unwrap(),
            TerminalType::Tmux,
        )
        .unwrap();
    manager
        .spawn(
            "2",
            "cmd",
            project.path.to_str().unwrap(),
            TerminalType::Tmux,
        )
        .unwrap();
    manager
        .spawn(
            "3",
            "cmd",
            project.path.to_str().unwrap(),
            TerminalType::Mock,
        )
        .unwrap();

    let sessions = manager.sessions();
    let terminals: Vec<_> = sessions.iter().map(|s| &s.terminal_type).collect();

    assert!(terminals.contains(&&TerminalType::Tmux));
    assert!(terminals.contains(&&TerminalType::Mock));
}

// US-16: Spawn Session Monitoring

#[test]
fn test_us16_sessions_lists_all_sessions() {
    let manager = MockTerminalManager::new();

    manager
        .spawn("task-1", "cmd1", "/p1", TerminalType::Tmux)
        .unwrap();
    manager
        .spawn("task-2", "cmd2", "/p2", TerminalType::Tmux)
        .unwrap();
    manager
        .spawn("task-3", "cmd3", "/p3", TerminalType::Tmux)
        .unwrap();

    let sessions = manager.sessions();
    assert_eq!(sessions.len(), 3);

    // All sessions should be running initially
    assert!(sessions.iter().all(|s| s.status == SessionStatus::Running));
}

#[test]
fn test_us16_session_status_tracking() {
    let manager = MockTerminalManager::new();

    let session_id = manager
        .spawn("task-1", "cmd", "/p", TerminalType::Tmux)
        .unwrap();

    // Initially running
    assert_eq!(manager.running_count(), 1);

    // Complete the session
    manager
        .complete_session(&session_id, Some("Task completed successfully".to_string()))
        .unwrap();

    // Now should be completed
    assert_eq!(manager.running_count(), 0);
    let session = manager.get_session(&session_id).unwrap();
    assert_eq!(session.status, SessionStatus::Completed);
    assert!(session.output.is_some());
}

#[test]
fn test_us16_session_metadata() {
    let manager = MockTerminalManager::new();

    let session_id = manager
        .spawn(
            "task-42",
            "claude code --prompt 'implement feature'",
            "/home/user/project",
            TerminalType::Tmux,
        )
        .unwrap();

    let session = manager.get_session(&session_id).unwrap();

    assert_eq!(session.task_id, "task-42");
    assert!(session.command.contains("implement feature"));
    assert_eq!(session.working_dir, "/home/user/project");
    assert_eq!(session.terminal_type, TerminalType::Tmux);
}

// US-17: Automatic Task Claiming

#[test]
fn test_us17_spawn_claim_marks_tasks_in_progress() {
    let project = TestProject::parallel_project();
    let phase = project.storage.load_active_group().unwrap();
    let manager = MockTerminalManager::new();

    // Get actionable task IDs
    let actionable = phase.get_actionable_tasks();
    let task_ids: Vec<String> = actionable.iter().take(2).map(|t| t.id.clone()).collect();

    // Spawn sessions for tasks
    for task_id in &task_ids {
        manager
            .spawn(
                task_id,
                &format!("claude code --task {}", task_id),
                project.path.to_str().unwrap(),
                TerminalType::Mock,
            )
            .unwrap();
    }

    // Now reload and mark as in-progress (claiming)
    let mut phase = project.storage.load_active_group().unwrap();
    for task_id in &task_ids {
        phase
            .get_task_mut(task_id)
            .unwrap()
            .set_status(TaskStatus::InProgress);
    }
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Verify tasks are now in-progress
    let phase = project.storage.load_active_group().unwrap();
    for task_id in &task_ids {
        let t = phase.get_task(task_id).unwrap();
        assert_eq!(t.status, TaskStatus::InProgress);
    }
}

#[test]
fn test_us17_claimed_tasks_show_assignee() {
    let project = TestProject::parallel_project();
    let mut phase = project.storage.load_active_group().unwrap();
    let manager = MockTerminalManager::new();

    // Spawn with claiming (assign to agent)
    let session_id = manager
        .spawn(
            "1",
            "claude code --task 1",
            project.path.to_str().unwrap(),
            TerminalType::Mock,
        )
        .unwrap();

    // Update task with assignment
    let task = phase.get_task_mut("1").unwrap();
    task.set_status(TaskStatus::InProgress);
    task.assigned_to = Some(format!("Agent-{}", session_id));

    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Verify assignment
    let phase = project.storage.load_active_group().unwrap();
    let task = phase.get_task("1").unwrap();
    assert!(task.assigned_to.is_some());
    assert!(task.assigned_to.as_ref().unwrap().starts_with("Agent-"));
}

#[test]
fn test_us17_spawn_only_pending_tasks() {
    let project = TestProject::stats_project();
    let phase = project.storage.load_active_group().unwrap();
    let manager = MockTerminalManager::new();

    // Only spawn tasks that are pending
    let pending_tasks: Vec<_> = phase
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .collect();

    for task in &pending_tasks {
        manager
            .spawn(
                &task.id,
                "cmd",
                project.path.to_str().unwrap(),
                TerminalType::Mock,
            )
            .unwrap();
    }

    // Should only spawn 3 (the pending ones, not in-progress or done)
    assert_eq!(manager.spawn_count(), 3);
}

// Error handling tests

#[test]
fn test_spawn_failure_handling() {
    let manager = MockTerminalManager::new().with_behavior(SpawnBehavior::AlwaysFail(
        "Terminal not available".to_string(),
    ));

    let result = manager.spawn("task-1", "cmd", "/p", TerminalType::Tmux);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Terminal not available"));
}

#[test]
fn test_spawn_partial_failure() {
    let manager = MockTerminalManager::new().fail_for_tasks(vec!["task-2".to_string()]);

    // task-1 should succeed
    assert!(manager
        .spawn("task-1", "cmd", "/p", TerminalType::Mock)
        .is_ok());

    // task-2 should fail
    assert!(manager
        .spawn("task-2", "cmd", "/p", TerminalType::Mock)
        .is_err());

    // task-3 should succeed
    assert!(manager
        .spawn("task-3", "cmd", "/p", TerminalType::Mock)
        .is_ok());

    // Should have 2 successful spawns
    assert_eq!(manager.spawn_count(), 2);
}

#[test]
fn test_terminate_session() {
    let manager = MockTerminalManager::new();

    let session_id = manager
        .spawn("task-1", "cmd", "/p", TerminalType::Mock)
        .unwrap();
    assert_eq!(manager.running_count(), 1);

    manager.terminate(&session_id).unwrap();

    let session = manager.get_session(&session_id).unwrap();
    assert_eq!(session.status, SessionStatus::Terminated);
    assert_eq!(manager.running_count(), 0);
}

#[test]
fn test_sessions_for_specific_task() {
    let manager = MockTerminalManager::new();

    // Spawn multiple sessions for different tasks
    manager
        .spawn("task-1", "cmd1", "/p", TerminalType::Mock)
        .unwrap();
    manager
        .spawn("task-1", "cmd2", "/p", TerminalType::Mock)
        .unwrap(); // Retry for same task
    manager
        .spawn("task-2", "cmd3", "/p", TerminalType::Mock)
        .unwrap();

    // Get sessions for task-1 only
    let task1_sessions = manager.sessions_for_task("task-1");
    assert_eq!(task1_sessions.len(), 2);

    let task2_sessions = manager.sessions_for_task("task-2");
    assert_eq!(task2_sessions.len(), 1);
}
