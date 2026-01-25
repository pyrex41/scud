//! Wave computation for parallel task execution.
//!
//! Computes execution waves using Kahn's algorithm (topological sort with level assignment),
//! grouping tasks that can run in parallel based on their dependencies.

use std::collections::{HashMap, HashSet};

use crate::models::Task;

/// A wave of tasks that can be executed in parallel
#[derive(Debug, Clone)]
pub struct Wave {
    /// Wave number (1-indexed)
    pub number: usize,
    /// Task IDs in this wave
    pub tasks: Vec<String>,
}

/// Result of wave computation
#[derive(Debug)]
pub struct WaveResult {
    /// Computed waves
    pub waves: Vec<Wave>,
    /// Tasks with circular dependencies (if any)
    pub circular_deps: Vec<String>,
}

/// Compute execution waves using Kahn's algorithm (topological sort with level assignment)
/// When processing tasks from multiple phases, task IDs are expected to be namespaced
pub fn compute_waves(tasks: &[&Task]) -> WaveResult {
    // Build a map from task pointer to its namespaced ID
    // This handles the case where multiple phases have tasks with the same local ID
    let task_ids: HashSet<String> = tasks.iter().map(|t| t.id.clone()).collect();

    // Build in-degree map (how many dependencies does each task have within our set?)
    let mut in_degree: HashMap<String, usize> = HashMap::new();
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

    for task in tasks {
        in_degree.entry(task.id.clone()).or_insert(0);

        for dep in &task.dependencies {
            // Only count dependencies that are in our actionable task set
            if task_ids.contains(dep) {
                *in_degree.entry(task.id.clone()).or_insert(0) += 1;
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(task.id.clone());
            }
        }
    }

    // Kahn's algorithm with wave tracking
    let mut waves: Vec<Wave> = Vec::new();
    let mut remaining = in_degree.clone();
    let mut wave_number = 1;
    let mut circular_deps = Vec::new();

    while !remaining.is_empty() {
        // Find all tasks with no remaining dependencies (in-degree = 0)
        let ready: Vec<String> = remaining
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(id, _)| id.clone())
            .collect();

        if ready.is_empty() {
            // Circular dependency detected - collect remaining tasks
            circular_deps = remaining.keys().cloned().collect();
            break;
        }

        // Remove ready tasks from remaining and update dependents
        for task_id in &ready {
            remaining.remove(task_id);

            if let Some(deps) = dependents.get(task_id) {
                for dep_id in deps {
                    if let Some(deg) = remaining.get_mut(dep_id) {
                        *deg = deg.saturating_sub(1);
                    }
                }
            }
        }

        waves.push(Wave {
            number: wave_number,
            tasks: ready,
        });
        wave_number += 1;
    }

    WaveResult {
        waves,
        circular_deps,
    }
}

/// Detect ID collisions when merging tasks from multiple phases
/// Returns a list of (local_id, Vec<tag>) for IDs that appear in multiple tags
pub fn detect_id_collisions(tasks: &[&Task]) -> Vec<(String, Vec<String>)> {
    let mut id_to_tags: HashMap<String, Vec<String>> = HashMap::new();

    for task in tasks {
        let local_id = task.local_id().to_string();
        let tag = task.epic_tag().unwrap_or("unknown").to_string();

        id_to_tags.entry(local_id).or_default().push(tag);
    }

    // Filter to only those with collisions (same local ID in multiple tags)
    let mut collisions: Vec<(String, Vec<String>)> = id_to_tags
        .into_iter()
        .filter(|(_, tags)| {
            // Dedupe tags and check if more than one unique tag
            let mut unique_tags: Vec<_> = tags.to_vec();
            unique_tags.sort();
            unique_tags.dedup();
            unique_tags.len() > 1
        })
        .map(|(id, mut tags)| {
            tags.sort();
            tags.dedup();
            (id, tags)
        })
        .collect();

    collisions.sort_by(|a, b| a.0.cmp(&b.0));
    collisions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Task;

    #[test]
    fn test_simple_linear_waves() {
        let task1 = Task::new("1".to_string(), "Task 1".to_string(), String::new());
        let mut task2 = Task::new("2".to_string(), "Task 2".to_string(), String::new());
        task2.dependencies = vec!["1".to_string()];
        let mut task3 = Task::new("3".to_string(), "Task 3".to_string(), String::new());
        task3.dependencies = vec!["2".to_string()];

        let tasks: Vec<&Task> = vec![&task1, &task2, &task3];
        let result = compute_waves(&tasks);

        assert_eq!(result.waves.len(), 3);
        assert!(result.circular_deps.is_empty());
        assert_eq!(result.waves[0].tasks, vec!["1"]);
        assert_eq!(result.waves[1].tasks, vec!["2"]);
        assert_eq!(result.waves[2].tasks, vec!["3"]);
    }

    #[test]
    fn test_parallel_waves() {
        let task1 = Task::new("1".to_string(), "Task 1".to_string(), String::new());
        let task2 = Task::new("2".to_string(), "Task 2".to_string(), String::new());
        let task3 = Task::new("3".to_string(), "Task 3".to_string(), String::new());

        let tasks: Vec<&Task> = vec![&task1, &task2, &task3];
        let result = compute_waves(&tasks);

        // All tasks can run in parallel (no dependencies)
        assert_eq!(result.waves.len(), 1);
        assert!(result.circular_deps.is_empty());
        assert_eq!(result.waves[0].tasks.len(), 3);
    }

    #[test]
    fn test_diamond_dependency() {
        //   1
        //  / \
        // 2   3
        //  \ /
        //   4
        let task1 = Task::new("1".to_string(), "Task 1".to_string(), String::new());
        let mut task2 = Task::new("2".to_string(), "Task 2".to_string(), String::new());
        task2.dependencies = vec!["1".to_string()];
        let mut task3 = Task::new("3".to_string(), "Task 3".to_string(), String::new());
        task3.dependencies = vec!["1".to_string()];
        let mut task4 = Task::new("4".to_string(), "Task 4".to_string(), String::new());
        task4.dependencies = vec!["2".to_string(), "3".to_string()];

        let tasks: Vec<&Task> = vec![&task1, &task2, &task3, &task4];
        let result = compute_waves(&tasks);

        assert_eq!(result.waves.len(), 3);
        assert!(result.circular_deps.is_empty());
        assert_eq!(result.waves[0].tasks, vec!["1"]);
        assert!(result.waves[1].tasks.contains(&"2".to_string()));
        assert!(result.waves[1].tasks.contains(&"3".to_string()));
        assert_eq!(result.waves[2].tasks, vec!["4"]);
    }

    #[test]
    fn test_circular_dependency_detected() {
        let mut task1 = Task::new("1".to_string(), "Task 1".to_string(), String::new());
        task1.dependencies = vec!["2".to_string()];
        let mut task2 = Task::new("2".to_string(), "Task 2".to_string(), String::new());
        task2.dependencies = vec!["1".to_string()];

        let tasks: Vec<&Task> = vec![&task1, &task2];
        let result = compute_waves(&tasks);

        assert!(result.waves.is_empty());
        assert_eq!(result.circular_deps.len(), 2);
    }

    #[test]
    fn test_external_dependency_ignored() {
        let task1 = Task::new("1".to_string(), "Task 1".to_string(), String::new());
        let mut task2 = Task::new("2".to_string(), "Task 2".to_string(), String::new());
        // Depends on task not in our set
        task2.dependencies = vec!["external:99".to_string()];

        let tasks: Vec<&Task> = vec![&task1, &task2];
        let result = compute_waves(&tasks);

        // Both can run in wave 1 since external dep is ignored
        assert_eq!(result.waves.len(), 1);
        assert_eq!(result.waves[0].tasks.len(), 2);
    }

    #[test]
    fn test_id_collision_detection() {
        // Two tasks with same local ID but different tags
        let task1 = Task::new("auth:1".to_string(), "Auth Task".to_string(), String::new());
        let task2 = Task::new("api:1".to_string(), "API Task".to_string(), String::new());
        let task3 = Task::new(
            "auth:2".to_string(),
            "Auth Task 2".to_string(),
            String::new(),
        );

        let tasks: Vec<&Task> = vec![&task1, &task2, &task3];
        let collisions = detect_id_collisions(&tasks);

        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].0, "1");
        assert!(collisions[0].1.contains(&"auth".to_string()));
        assert!(collisions[0].1.contains(&"api".to_string()));
    }

    #[test]
    fn test_no_id_collisions() {
        let task1 = Task::new("auth:1".to_string(), "Auth Task".to_string(), String::new());
        let task2 = Task::new("api:2".to_string(), "API Task".to_string(), String::new());

        let tasks: Vec<&Task> = vec![&task1, &task2];
        let collisions = detect_id_collisions(&tasks);

        assert!(collisions.is_empty());
    }
}
