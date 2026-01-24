//! E2E tests for swarm with extension-based agent spawning
//!
//! Tests the full workflow from task loading to agent spawning to completion:
//! - Wave computation and execution order
//! - Status updates through storage
//! - Extension-based subprocess spawning
//! - Session state tracking

use crate::e2e::fixtures::TestProject;
use scud::commands::swarm::session::{
    RoundState, SwarmSession, WaveAgent, WaveExecutionResult, WaveState, WaveSummary,
};
use scud::extensions::runner::AgentResult;
use scud::models::task::{Task, TaskStatus};

// ============================================================================
// Wave Computation Tests
// ============================================================================

#[test]
fn test_wave_agent_creation() {
    let task = Task::new(
        "task:1".to_string(),
        "Test task".to_string(),
        "Description".to_string(),
    );
    let agent = WaveAgent::new(task.clone(), "test-tag");

    assert_eq!(agent.task_id(), "task:1");
    assert_eq!(agent.tag, "test-tag");
    assert_eq!(agent.task.title, "Test task");
}

#[test]
fn test_wave_agent_from_task_pairs() {
    let task1 = Task::new(
        "task:1".to_string(),
        "Task 1".to_string(),
        "Desc 1".to_string(),
    );
    let task2 = Task::new(
        "task:2".to_string(),
        "Task 2".to_string(),
        "Desc 2".to_string(),
    );
    let task3 = Task::new(
        "task:3".to_string(),
        "Task 3".to_string(),
        "Desc 3".to_string(),
    );

    let pairs = vec![
        (task1, "phase-a".to_string()),
        (task2, "phase-a".to_string()),
        (task3, "phase-b".to_string()),
    ];

    let agents = WaveAgent::from_task_pairs(pairs);

    assert_eq!(agents.len(), 3);
    assert_eq!(agents[0].task_id(), "task:1");
    assert_eq!(agents[0].tag, "phase-a");
    assert_eq!(agents[2].task_id(), "task:3");
    assert_eq!(agents[2].tag, "phase-b");
}

// ============================================================================
// Wave Execution Result Tests
// ============================================================================

#[test]
fn test_wave_execution_result_all_succeeded() {
    let round_state = RoundState::new(0);
    let result = WaveExecutionResult {
        round_state,
        agent_results: vec![
            AgentResult {
                task_id: "task:1".to_string(),
                success: true,
                exit_code: Some(0),
                output: "completed".to_string(),
                duration_ms: 1000,
            },
            AgentResult {
                task_id: "task:2".to_string(),
                success: true,
                exit_code: Some(0),
                output: "completed".to_string(),
                duration_ms: 1500,
            },
        ],
    };

    assert!(result.all_succeeded());
    assert_eq!(result.successful_task_ids(), vec!["task:1", "task:2"]);
    assert!(result.failed_task_ids().is_empty());
    assert_eq!(result.total_duration_ms(), 1500); // max duration
}

#[test]
fn test_wave_execution_result_with_failures() {
    let round_state = RoundState::new(0);
    let result = WaveExecutionResult {
        round_state,
        agent_results: vec![
            AgentResult {
                task_id: "task:1".to_string(),
                success: true,
                exit_code: Some(0),
                output: "completed".to_string(),
                duration_ms: 1000,
            },
            AgentResult {
                task_id: "task:2".to_string(),
                success: false,
                exit_code: Some(1),
                output: "error".to_string(),
                duration_ms: 500,
            },
            AgentResult {
                task_id: "task:3".to_string(),
                success: false,
                exit_code: None,
                output: "spawn failed".to_string(),
                duration_ms: 0,
            },
        ],
    };

    assert!(!result.all_succeeded());
    assert_eq!(result.successful_task_ids(), vec!["task:1"]);
    assert_eq!(result.failed_task_ids(), vec!["task:2", "task:3"]);
}

// ============================================================================
// Wave State Integration Tests
// ============================================================================

#[test]
fn test_wave_state_from_execution_result() {
    let mut round_state = RoundState::new(0);
    round_state.task_ids = vec!["task:1".to_string(), "task:2".to_string()];
    round_state.tags = vec!["phase-a".to_string(), "phase-a".to_string()];

    let result = WaveExecutionResult {
        round_state,
        agent_results: vec![
            AgentResult {
                task_id: "task:1".to_string(),
                success: true,
                exit_code: Some(0),
                output: "done".to_string(),
                duration_ms: 1000,
            },
            AgentResult {
                task_id: "task:2".to_string(),
                success: true,
                exit_code: Some(0),
                output: "done".to_string(),
                duration_ms: 1200,
            },
        ],
    };

    let wave_state = WaveState::from_execution_result(1, result);

    assert_eq!(wave_state.wave_number, 1);
    assert_eq!(wave_state.rounds.len(), 1);
    assert_eq!(wave_state.all_task_ids(), vec!["task:1", "task:2"]);

    let task_tags = wave_state.task_tags();
    assert_eq!(task_tags.len(), 2);
    assert!(task_tags.contains(&("task:1".to_string(), "phase-a".to_string())));
}

#[test]
fn test_wave_state_multiple_rounds() {
    let mut wave_state = WaveState::new(1);

    // Round 1
    let mut round1 = RoundState::new(0);
    round1.task_ids = vec!["task:1".to_string(), "task:2".to_string()];
    round1.tags = vec!["phase-a".to_string(), "phase-a".to_string()];

    let result1 = WaveExecutionResult {
        round_state: round1,
        agent_results: vec![
            AgentResult {
                task_id: "task:1".to_string(),
                success: true,
                exit_code: Some(0),
                output: "".to_string(),
                duration_ms: 100,
            },
            AgentResult {
                task_id: "task:2".to_string(),
                success: true,
                exit_code: Some(0),
                output: "".to_string(),
                duration_ms: 100,
            },
        ],
    };
    wave_state.apply_execution_result(result1);

    // Round 2
    let mut round2 = RoundState::new(1);
    round2.task_ids = vec!["task:3".to_string()];
    round2.tags = vec!["phase-b".to_string()];

    let result2 = WaveExecutionResult {
        round_state: round2,
        agent_results: vec![AgentResult {
            task_id: "task:3".to_string(),
            success: true,
            exit_code: Some(0),
            output: "".to_string(),
            duration_ms: 100,
        }],
    };
    wave_state.apply_execution_result(result2);

    assert_eq!(wave_state.rounds.len(), 2);
    assert_eq!(
        wave_state.all_task_ids(),
        vec!["task:1", "task:2", "task:3"]
    );

    let task_tags = wave_state.task_tags();
    assert_eq!(task_tags.len(), 3);
    assert!(task_tags.contains(&("task:3".to_string(), "phase-b".to_string())));
}

// ============================================================================
// Swarm Session State Tracking Tests
// ============================================================================

#[test]
fn test_swarm_session_creation() {
    let session = SwarmSession::new("test-session", "test-tag", "extensions", "/tmp/project", 5);

    assert_eq!(session.session_name, "test-session");
    assert_eq!(session.tag, "test-tag");
    assert_eq!(session.terminal, "extensions");
    assert_eq!(session.round_size, 5);
    assert!(session.waves.is_empty());
    assert!(session.completed_at.is_none());
}

#[test]
fn test_swarm_session_wave_tracking() {
    let mut session = SwarmSession::new("test-session", "phase-a", "extensions", "/tmp", 3);

    // Wave 1
    let mut wave1 = WaveState::new(1);
    let mut round1 = RoundState::new(0);
    round1.task_ids = vec!["1".to_string(), "2".to_string()];
    wave1.rounds.push(round1);
    wave1.summary = Some(WaveSummary {
        wave_number: 1,
        tasks_completed: vec!["1".to_string(), "2".to_string()],
        files_changed: vec!["src/main.rs".to_string()],
    });
    session.waves.push(wave1);

    // Wave 2 (with summary for get_previous_summary test)
    let mut wave2 = WaveState::new(2);
    let mut round2 = RoundState::new(0);
    round2.task_ids = vec!["3".to_string()];
    wave2.rounds.push(round2);
    wave2.summary = Some(WaveSummary {
        wave_number: 2,
        tasks_completed: vec!["3".to_string()],
        files_changed: vec![],
    });
    session.waves.push(wave2);

    assert_eq!(session.total_tasks(), 3);
    assert_eq!(session.total_failures(), 0);

    // get_previous_summary returns the last wave's summary
    let summary = session.get_previous_summary();
    assert!(summary.is_some());
    assert!(summary.unwrap().contains("Wave 2"));
}

#[test]
fn test_swarm_session_failure_tracking() {
    let mut session = SwarmSession::new("test-session", "phase-a", "extensions", "/tmp", 3);

    let mut wave = WaveState::new(1);
    let mut round = RoundState::new(0);
    round.task_ids = vec!["1".to_string(), "2".to_string()];
    round.failures = vec!["3".to_string()]; // One failed to spawn
    wave.rounds.push(round);
    session.waves.push(wave);

    assert_eq!(session.total_tasks(), 2);
    assert_eq!(session.total_failures(), 1);
}

// ============================================================================
// Storage Integration Tests (Task Status Updates)
// ============================================================================

#[test]
fn test_status_updates_through_storage() {
    let project = TestProject::simple_project();
    let phase = project.storage.load_active_group().unwrap();

    // Initially all pending
    assert_eq!(phase.get_task("1").unwrap().status, TaskStatus::Pending);
    assert_eq!(phase.get_task("2").unwrap().status, TaskStatus::Pending);
    assert_eq!(phase.get_task("3").unwrap().status, TaskStatus::Pending);

    // Mark task 1 as in-progress (simulating agent spawn)
    let mut phase = project.storage.load_active_group().unwrap();
    if let Some(task) = phase.get_task_mut("1") {
        task.set_status(TaskStatus::InProgress);
    }
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Verify persistence
    let phase = project.storage.load_active_group().unwrap();
    assert_eq!(phase.get_task("1").unwrap().status, TaskStatus::InProgress);

    // Mark task 1 as done (simulating agent completion)
    let mut phase = project.storage.load_active_group().unwrap();
    if let Some(task) = phase.get_task_mut("1") {
        task.set_status(TaskStatus::Done);
    }
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Verify final state
    let phase = project.storage.load_active_group().unwrap();
    assert_eq!(phase.get_task("1").unwrap().status, TaskStatus::Done);
}

#[test]
fn test_parallel_task_status_updates() {
    let project = TestProject::parallel_project();

    // Mark both independent tasks as in-progress simultaneously
    let mut phase = project.storage.load_active_group().unwrap();
    if let Some(task) = phase.get_task_mut("1") {
        task.set_status(TaskStatus::InProgress);
    }
    if let Some(task) = phase.get_task_mut("2") {
        task.set_status(TaskStatus::InProgress);
    }
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Verify both are in-progress
    let phase = project.storage.load_active_group().unwrap();
    assert_eq!(phase.get_task("1").unwrap().status, TaskStatus::InProgress);
    assert_eq!(phase.get_task("2").unwrap().status, TaskStatus::InProgress);

    // Complete both
    let mut phase = project.storage.load_active_group().unwrap();
    if let Some(task) = phase.get_task_mut("1") {
        task.set_status(TaskStatus::Done);
    }
    if let Some(task) = phase.get_task_mut("2") {
        task.set_status(TaskStatus::Done);
    }
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Task 3 should still be pending (depends on both 1 and 2)
    let phase = project.storage.load_active_group().unwrap();
    assert_eq!(phase.get_task("1").unwrap().status, TaskStatus::Done);
    assert_eq!(phase.get_task("2").unwrap().status, TaskStatus::Done);
    assert_eq!(phase.get_task("3").unwrap().status, TaskStatus::Pending);
}

// ============================================================================
// Wave Execution Order Tests
// ============================================================================

#[test]
fn test_wave_project_dependency_order() {
    let project = TestProject::wave_project();
    let phase = project.storage.load_active_group().unwrap();

    // Wave 1: Tasks 1, 2 (no deps) - should be actionable
    let task1 = phase.get_task("1").unwrap();
    let task2 = phase.get_task("2").unwrap();
    assert!(task1.dependencies.is_empty());
    assert!(task2.dependencies.is_empty());

    // Wave 2: Tasks 3, 4 (depend on wave 1) - blocked initially
    let task3 = phase.get_task("3").unwrap();
    let task4 = phase.get_task("4").unwrap();
    assert!(task3.dependencies.contains(&"1".to_string()));
    assert!(task4.dependencies.contains(&"2".to_string()));

    // Wave 3: Task 5 (depends on wave 2)
    let task5 = phase.get_task("5").unwrap();
    assert!(task5.dependencies.contains(&"3".to_string()));
    assert!(task5.dependencies.contains(&"4".to_string()));
}

#[test]
fn test_simulate_wave_execution_flow() {
    let project = TestProject::wave_project();

    // Wave 1: Execute tasks 1 and 2
    let mut phase = project.storage.load_active_group().unwrap();
    if let Some(task) = phase.get_task_mut("1") {
        task.set_status(TaskStatus::InProgress);
    }
    if let Some(task) = phase.get_task_mut("2") {
        task.set_status(TaskStatus::InProgress);
    }
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Complete wave 1
    let mut phase = project.storage.load_active_group().unwrap();
    if let Some(task) = phase.get_task_mut("1") {
        task.set_status(TaskStatus::Done);
    }
    if let Some(task) = phase.get_task_mut("2") {
        task.set_status(TaskStatus::Done);
    }
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Now wave 2 tasks (3, 4) should be unblocked
    // Verify dependencies are satisfied
    let phase = project.storage.load_active_group().unwrap();
    let task3 = phase.get_task("3").unwrap();
    let task4 = phase.get_task("4").unwrap();

    // Task 3 depends on 1 (now done)
    assert_eq!(
        phase.get_task(&task3.dependencies[0]).unwrap().status,
        TaskStatus::Done
    );
    // Task 4 depends on 2 (now done)
    assert_eq!(
        phase.get_task(&task4.dependencies[0]).unwrap().status,
        TaskStatus::Done
    );

    // Execute wave 2
    let mut phase = project.storage.load_active_group().unwrap();
    if let Some(task) = phase.get_task_mut("3") {
        task.set_status(TaskStatus::Done);
    }
    if let Some(task) = phase.get_task_mut("4") {
        task.set_status(TaskStatus::Done);
    }
    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Now task 5 (wave 3) should be unblocked
    let phase = project.storage.load_active_group().unwrap();
    let task5 = phase.get_task("5").unwrap();

    // Both dependencies done
    assert_eq!(
        phase.get_task(&task5.dependencies[0]).unwrap().status,
        TaskStatus::Done
    );
    assert_eq!(
        phase.get_task(&task5.dependencies[1]).unwrap().status,
        TaskStatus::Done
    );
}

// ============================================================================
// Round State Tests
// ============================================================================

#[test]
fn test_round_state_lifecycle() {
    let mut round = RoundState::new(0);

    assert_eq!(round.round_number, 0);
    assert!(round.task_ids.is_empty());
    assert!(round.tags.is_empty());
    assert!(round.failures.is_empty());
    assert!(round.completed_at.is_none());

    // Add tasks
    round.task_ids.push("task:1".to_string());
    round.task_ids.push("task:2".to_string());
    round.tags.push("phase-a".to_string());
    round.tags.push("phase-a".to_string());

    // Mark complete
    round.mark_complete();
    assert!(round.completed_at.is_some());
}

#[test]
fn test_round_state_with_failures() {
    let mut round = RoundState::new(0);

    // Some tasks spawned successfully
    round.task_ids.push("task:1".to_string());
    round.tags.push("phase-a".to_string());

    // Some failed to spawn
    round.failures.push("task:2".to_string());

    assert_eq!(round.task_ids.len(), 1);
    assert_eq!(round.failures.len(), 1);
}

// ============================================================================
// Wave Summary Tests
// ============================================================================

#[test]
fn test_wave_summary_to_text() {
    let summary = WaveSummary {
        wave_number: 1,
        tasks_completed: vec!["1".to_string(), "2".to_string(), "3".to_string()],
        files_changed: vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "tests/test.rs".to_string(),
        ],
    };

    let text = summary.to_text();
    assert!(text.contains("Wave 1"));
    assert!(text.contains("3 task(s)"));
    assert!(text.contains("src/main.rs"));
}

#[test]
fn test_wave_summary_truncates_many_files() {
    let summary = WaveSummary {
        wave_number: 1,
        tasks_completed: vec!["1".to_string()],
        files_changed: vec![
            "file1.rs".to_string(),
            "file2.rs".to_string(),
            "file3.rs".to_string(),
            "file4.rs".to_string(),
            "file5.rs".to_string(),
            "file6.rs".to_string(),
            "file7.rs".to_string(),
        ],
    };

    let text = summary.to_text();
    assert!(text.contains("and 2 more"));
}

// ============================================================================
// Session Persistence Tests
// ============================================================================

#[test]
fn test_session_save_and_load() {
    use scud::commands::swarm::session::{load_session, save_session};

    let project = TestProject::new();

    let mut session = SwarmSession::new("test-swarm", "test-tag", "extensions", "/tmp", 5);

    // Add a wave
    let mut wave = WaveState::new(1);
    let mut round = RoundState::new(0);
    round.task_ids = vec!["task:1".to_string()];
    round.tags = vec!["test-tag".to_string()];
    wave.rounds.push(round);
    session.waves.push(wave);

    // Save
    save_session(Some(&project.path), &session).expect("Failed to save session");

    // Load
    let loaded = load_session(Some(&project.path), "test-swarm").expect("Failed to load session");

    assert_eq!(loaded.session_name, "test-swarm");
    assert_eq!(loaded.tag, "test-tag");
    assert_eq!(loaded.waves.len(), 1);
    assert_eq!(loaded.waves[0].rounds[0].task_ids, vec!["task:1"]);
}

#[test]
fn test_session_list() {
    use scud::commands::swarm::session::{list_sessions, save_session};

    let project = TestProject::new();

    // Save multiple sessions
    let session1 = SwarmSession::new("session-1", "tag-a", "extensions", "/tmp", 5);
    let session2 = SwarmSession::new("session-2", "tag-b", "extensions", "/tmp", 5);

    save_session(Some(&project.path), &session1).expect("Failed to save session 1");
    save_session(Some(&project.path), &session2).expect("Failed to save session 2");

    // List
    let sessions = list_sessions(Some(&project.path)).expect("Failed to list sessions");

    assert!(sessions.contains(&"session-1".to_string()));
    assert!(sessions.contains(&"session-2".to_string()));
}

// ============================================================================
// Full Workflow Simulation Test
// ============================================================================

#[test]
fn test_full_swarm_workflow_simulation() {
    let project = TestProject::wave_project();

    // Create a swarm session
    let mut session = SwarmSession::new(
        "e2e-test-session",
        &project.tag(),
        "extensions",
        &project.path.to_string_lossy(),
        2, // round_size = 2
    );

    // === Wave 1: Tasks 1 and 2 (no dependencies) ===
    let mut wave1 = WaveState::new(1);

    // Round 1 of wave 1
    let mut round1_1 = RoundState::new(0);
    round1_1.task_ids = vec!["1".to_string(), "2".to_string()];
    round1_1.tags = vec![project.tag(), project.tag()];

    // Simulate agent execution
    let mut phase = project.storage.load_active_group().unwrap();
    phase.get_task_mut("1").unwrap().set_status(TaskStatus::InProgress);
    phase.get_task_mut("2").unwrap().set_status(TaskStatus::InProgress);
    project.storage.update_group(&project.tag(), &phase).unwrap();

    // Simulate completion
    let mut phase = project.storage.load_active_group().unwrap();
    phase.get_task_mut("1").unwrap().set_status(TaskStatus::Done);
    phase.get_task_mut("2").unwrap().set_status(TaskStatus::Done);
    project.storage.update_group(&project.tag(), &phase).unwrap();

    round1_1.mark_complete();
    wave1.rounds.push(round1_1);
    wave1.summary = Some(WaveSummary {
        wave_number: 1,
        tasks_completed: vec!["1".to_string(), "2".to_string()],
        files_changed: vec![],
    });
    wave1.mark_complete();
    session.waves.push(wave1);

    // === Wave 2: Tasks 3 and 4 (depend on wave 1) ===
    let mut wave2 = WaveState::new(2);

    let mut round2_1 = RoundState::new(0);
    round2_1.task_ids = vec!["3".to_string(), "4".to_string()];
    round2_1.tags = vec![project.tag(), project.tag()];

    let mut phase = project.storage.load_active_group().unwrap();
    phase.get_task_mut("3").unwrap().set_status(TaskStatus::InProgress);
    phase.get_task_mut("4").unwrap().set_status(TaskStatus::InProgress);
    project.storage.update_group(&project.tag(), &phase).unwrap();

    let mut phase = project.storage.load_active_group().unwrap();
    phase.get_task_mut("3").unwrap().set_status(TaskStatus::Done);
    phase.get_task_mut("4").unwrap().set_status(TaskStatus::Done);
    project.storage.update_group(&project.tag(), &phase).unwrap();

    round2_1.mark_complete();
    wave2.rounds.push(round2_1);
    wave2.summary = Some(WaveSummary {
        wave_number: 2,
        tasks_completed: vec!["3".to_string(), "4".to_string()],
        files_changed: vec![],
    });
    wave2.mark_complete();
    session.waves.push(wave2);

    // === Wave 3: Task 5 (depends on wave 2) ===
    let mut wave3 = WaveState::new(3);

    let mut round3_1 = RoundState::new(0);
    round3_1.task_ids = vec!["5".to_string()];
    round3_1.tags = vec![project.tag()];

    let mut phase = project.storage.load_active_group().unwrap();
    phase.get_task_mut("5").unwrap().set_status(TaskStatus::InProgress);
    project.storage.update_group(&project.tag(), &phase).unwrap();

    let mut phase = project.storage.load_active_group().unwrap();
    phase.get_task_mut("5").unwrap().set_status(TaskStatus::Done);
    project.storage.update_group(&project.tag(), &phase).unwrap();

    round3_1.mark_complete();
    wave3.rounds.push(round3_1);
    wave3.summary = Some(WaveSummary {
        wave_number: 3,
        tasks_completed: vec!["5".to_string()],
        files_changed: vec![],
    });
    wave3.mark_complete();
    session.waves.push(wave3);

    // === Verify Final State ===
    session.mark_complete();

    // Session stats
    assert_eq!(session.waves.len(), 3);
    assert_eq!(session.total_tasks(), 5);
    assert_eq!(session.total_failures(), 0);

    // All tasks done
    let phase = project.storage.load_active_group().unwrap();
    for task in &phase.tasks {
        assert_eq!(
            task.status,
            TaskStatus::Done,
            "Task {} should be done",
            task.id
        );
    }

    // Verify wave order
    assert_eq!(session.waves[0].all_task_ids(), vec!["1", "2"]);
    assert_eq!(session.waves[1].all_task_ids(), vec!["3", "4"]);
    assert_eq!(session.waves[2].all_task_ids(), vec!["5"]);
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_execution_with_spawn_failures() {
    let project = TestProject::simple_project();

    let mut session = SwarmSession::new(
        "failure-test",
        &project.tag(),
        "extensions",
        &project.path.to_string_lossy(),
        3,
    );

    // Wave with spawn failure
    let mut wave = WaveState::new(1);
    let mut round = RoundState::new(0);
    round.task_ids = vec!["1".to_string()]; // Task 1 spawned
    round.tags = vec![project.tag()];
    round.failures = vec!["2".to_string()]; // Task 2 failed to spawn
    round.mark_complete();
    wave.rounds.push(round);
    session.waves.push(wave);

    assert_eq!(session.total_tasks(), 1);
    assert_eq!(session.total_failures(), 1);
}

#[test]
fn test_task_failure_status() {
    let project = TestProject::simple_project();

    // Mark task as failed
    let mut phase = project.storage.load_active_group().unwrap();
    phase.get_task_mut("1").unwrap().set_status(TaskStatus::Failed);
    project.storage.update_group(&project.tag(), &phase).unwrap();

    // Verify
    let phase = project.storage.load_active_group().unwrap();
    assert_eq!(phase.get_task("1").unwrap().status, TaskStatus::Failed);
}

// ============================================================================
// Agent Type Tests
// ============================================================================

#[test]
fn test_wave_agent_with_agent_type() {
    let mut task = Task::new(
        "task:1".to_string(),
        "Build feature".to_string(),
        "Description".to_string(),
    );
    task.agent_type = Some("builder".to_string());

    let agent = WaveAgent::new(task, "test-tag");

    assert_eq!(agent.task.agent_type, Some("builder".to_string()));
}

#[test]
fn test_wave_agent_without_agent_type() {
    let task = Task::new(
        "task:1".to_string(),
        "Simple task".to_string(),
        "Description".to_string(),
    );

    let agent = WaveAgent::new(task, "test-tag");

    assert!(agent.task.agent_type.is_none());
}
