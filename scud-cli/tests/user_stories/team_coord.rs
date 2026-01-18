//! Team Coordination Tests (US-6 to US-9)
//!
//! Tests for team coordination features:
//! - US-6: Task Assignment Tracking (assign, who-is)
//! - US-7: Parallel Execution Planning (waves)
//! - US-8: Project Statistics Dashboard (stats)
//! - US-9: Dependency Visualization (mermaid)

use crate::e2e::fixtures::TestProject;

// US-6: Task Assignment Tracking

#[test]
fn test_us6_assign_sets_assignee() {
    let project = TestProject::simple_project();
    let mut phase = project.storage.load_active_group().unwrap();

    // Assign task to user
    let task = phase.get_task_mut("1").unwrap();
    task.assigned_to = Some("Alice".to_string());

    project
        .storage
        .update_group(&project.tag(), &phase)
        .unwrap();

    // Verify assignment persisted
    let phase = project.storage.load_active_group().unwrap();
    let task = phase.get_task("1").unwrap();
    assert_eq!(task.assigned_to, Some("Alice".to_string()));
}

#[test]
fn test_us6_assigned_project_structure() {
    let project = TestProject::assigned_project();
    let phase = project.storage.load_active_group().unwrap();

    // Count assigned tasks
    let assigned_count = phase
        .tasks
        .iter()
        .filter(|t| t.assigned_to.is_some())
        .count();

    assert_eq!(assigned_count, 2);
}

#[test]
fn test_us6_find_tasks_by_assignee() {
    let project = TestProject::assigned_project();
    let phase = project.storage.load_active_group().unwrap();

    // Find Alice's tasks
    let alice_tasks: Vec<_> = phase
        .tasks
        .iter()
        .filter(|t| t.assigned_to.as_deref() == Some("Alice"))
        .collect();

    assert_eq!(alice_tasks.len(), 1);
    assert_eq!(alice_tasks[0].title, "Task for Alice");
}

// US-7: Parallel Execution Planning (Waves)

#[test]
fn test_us7_wave_project_has_correct_structure() {
    let project = TestProject::wave_project();
    let phase = project.storage.load_active_group().unwrap();

    // Should have 5 tasks total
    assert_eq!(phase.tasks.len(), 5);

    // Tasks 1, 2 should have no dependencies (wave 1)
    let t1 = phase.get_task("1").unwrap();
    let t2 = phase.get_task("2").unwrap();
    assert!(t1.dependencies.is_empty());
    assert!(t2.dependencies.is_empty());

    // Task 5 should depend on 3 and 4 (wave 3)
    let t5 = phase.get_task("5").unwrap();
    assert_eq!(t5.dependencies.len(), 2);
}

#[test]
fn test_us7_parallel_tasks_identified() {
    let project = TestProject::parallel_project();
    let phase = project.storage.load_active_group().unwrap();

    // Tasks 1 and 2 should be independent (can run in parallel)
    let t1 = phase.get_task("1").unwrap();
    let t2 = phase.get_task("2").unwrap();

    assert!(t1.dependencies.is_empty());
    assert!(t2.dependencies.is_empty());

    // Get actionable tasks - both should be available
    let actionable = phase.get_actionable_tasks();
    let actionable_ids: Vec<_> = actionable.iter().map(|t| t.id.as_str()).collect();

    assert!(actionable_ids.contains(&"1"));
    assert!(actionable_ids.contains(&"2"));
}

#[test]
fn test_us7_diamond_dependency_structure() {
    let project = TestProject::parallel_project();
    let phase = project.storage.load_active_group().unwrap();

    // Task 3 depends on both 1 and 2
    let t3 = phase.get_task("3").unwrap();
    assert!(t3.dependencies.contains(&"1".to_string()));
    assert!(t3.dependencies.contains(&"2".to_string()));
}

// US-8: Project Statistics Dashboard

#[test]
fn test_us8_stats_shows_status_counts() {
    let project = TestProject::stats_project();
    let phase = project.storage.load_active_group().unwrap();
    let stats = phase.get_stats();

    assert_eq!(stats.pending, 3);
    assert_eq!(stats.done, 2);
    assert_eq!(stats.in_progress, 1);
    assert_eq!(stats.total, 6);
}

#[test]
fn test_us8_stats_completion_percentage() {
    let project = TestProject::stats_project();
    let phase = project.storage.load_active_group().unwrap();
    let stats = phase.get_stats();

    // 2 out of 6 done = 33.33%
    let completion_pct = (stats.done as f64 / stats.total as f64) * 100.0;
    assert!((completion_pct - 33.33).abs() < 1.0);
}

#[test]
fn test_us8_empty_phase_stats() {
    let project = TestProject::new();
    let empty_phase = scud::models::Phase::new("empty".to_string());

    let mut tasks = std::collections::HashMap::new();
    tasks.insert("empty".to_string(), empty_phase);
    project.storage.save_tasks(&tasks).unwrap();
    project.storage.set_active_group("empty").unwrap();

    let phase = project.storage.load_active_group().unwrap();
    let stats = phase.get_stats();

    assert_eq!(stats.total, 0);
    assert_eq!(stats.done, 0);
    assert_eq!(stats.pending, 0);
}

// US-9: Dependency Visualization

#[test]
fn test_us9_simple_dependency_chain() {
    let project = TestProject::simple_project();
    let phase = project.storage.load_active_group().unwrap();

    // Verify dependency chain: 1 -> 2 -> 3
    let t2 = phase.get_task("2").unwrap();
    let t3 = phase.get_task("3").unwrap();

    assert!(t2.dependencies.contains(&"1".to_string()));
    assert!(t3.dependencies.contains(&"2".to_string()));
}

#[test]
fn test_us9_dependency_structure_for_mermaid() {
    let project = TestProject::wave_project();
    let phase = project.storage.load_active_group().unwrap();

    // Collect all edges for mermaid generation
    let mut edges: Vec<(String, String)> = Vec::new();

    for task in &phase.tasks {
        for dep in &task.dependencies {
            edges.push((dep.clone(), task.id.clone()));
        }
    }

    // Should have edges: 1->3, 2->4, 3->5, 4->5
    assert!(edges.contains(&("1".to_string(), "3".to_string())));
    assert!(edges.contains(&("2".to_string(), "4".to_string())));
    assert!(edges.contains(&("3".to_string(), "5".to_string())));
    assert!(edges.contains(&("4".to_string(), "5".to_string())));
}

#[test]
fn test_us9_cross_phase_deps_visible() {
    let project = TestProject::multi_phase_project();
    let tasks = project.storage.load_tasks().unwrap();

    // Collect cross-phase dependencies
    let mut cross_phase_deps: Vec<(String, String, String)> = Vec::new();

    for (phase_name, phase) in &tasks {
        for task in &phase.tasks {
            for dep in &task.dependencies {
                if dep.contains(':') {
                    cross_phase_deps.push((phase_name.clone(), task.id.clone(), dep.clone()));
                }
            }
        }
    }

    // Should have phase2:1 -> phase1:1
    assert!(!cross_phase_deps.is_empty());
}
