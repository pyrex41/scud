//! SCUD Library Usage Example
//!
//! This example demonstrates key library features including:
//! - Loading and managing tasks
//! - Finding ready tasks (dependency resolution)
//! - Running backpressure validation
//! - Working with phases and task status
//!
//! Run with: cargo run --example library_usage

use scud::backpressure::{run_validation, BackpressureConfig};
use scud::models::{Phase, Priority, Task, TaskStatus};
use scud::storage::Storage;
use std::path::Path;

fn main() {
    println!("=== SCUD Library Usage Example ===\n");

    // Run each demonstration
    demonstrate_task_creation();
    demonstrate_phase_operations();
    demonstrate_dependency_resolution();
    demonstrate_storage_operations();
    demonstrate_backpressure_validation();
}

/// Demonstrates creating and configuring tasks
fn demonstrate_task_creation() {
    println!("--- Task Creation ---\n");

    // Create a new task with basic info
    let mut task = Task::new(
        "1".to_string(),
        "Implement user authentication".to_string(),
        "Add login/logout functionality with JWT tokens".to_string(),
    );

    // Configure task properties
    task.complexity = 8; // Fibonacci complexity (1, 2, 3, 5, 8, 13, 21, 34)
    task.priority = Priority::High;
    task.dependencies = vec!["setup:1".to_string()]; // Cross-phase dependency

    println!("Created task: {} - {}", task.id, task.title);
    println!("  Complexity: {}", task.complexity);
    println!("  Priority: {:?}", task.priority);
    println!("  Dependencies: {:?}", task.dependencies);

    // Update task status
    task.set_status(TaskStatus::InProgress);
    println!("  Status: {:?}\n", task.status);
}

/// Demonstrates working with phases (task collections)
fn demonstrate_phase_operations() {
    println!("--- Phase Operations ---\n");

    // Create a new phase (tag/group of tasks)
    let mut phase = Phase::new("auth".to_string());

    // Add tasks to the phase
    let mut task1 = Task::new(
        "1".to_string(),
        "Setup database schema".to_string(),
        "Create user and session tables".to_string(),
    );
    task1.complexity = 5;
    task1.set_status(TaskStatus::Done);

    let mut task2 = Task::new(
        "2".to_string(),
        "Implement login endpoint".to_string(),
        "POST /api/login with email/password".to_string(),
    );
    task2.complexity = 5;
    task2.dependencies = vec!["1".to_string()];

    let mut task3 = Task::new(
        "3".to_string(),
        "Add JWT middleware".to_string(),
        "Protect routes with token verification".to_string(),
    );
    task3.complexity = 8;
    task3.dependencies = vec!["2".to_string()];

    phase.add_task(task1);
    phase.add_task(task2);
    phase.add_task(task3);

    // Get phase statistics
    let stats = phase.get_stats();
    println!("Phase '{}' statistics:", phase.name);
    println!("  Total tasks: {}", stats.total);
    println!("  Done: {}", stats.done);
    println!("  Pending: {}", stats.pending);
    println!("  In progress: {}", stats.in_progress);
    println!("  Total complexity: {}", stats.total_complexity);

    // Retrieve a specific task
    if let Some(task) = phase.get_task("2") {
        println!("\nRetrieved task 2: {}", task.title);
    }

    // Modify a task in place
    if let Some(task) = phase.get_task_mut("2") {
        task.set_status(TaskStatus::InProgress);
        println!("Updated task 2 status to: {:?}\n", task.status);
    }
}

/// Demonstrates finding the next available task based on dependencies
fn demonstrate_dependency_resolution() {
    println!("--- Dependency Resolution ---\n");

    let mut phase = Phase::new("api".to_string());

    // Task 1: No dependencies, done
    let mut task1 = Task::new(
        "1".to_string(),
        "Initialize project".to_string(),
        "Setup cargo project".to_string(),
    );
    task1.set_status(TaskStatus::Done);

    // Task 2: Depends on task 1, pending
    let mut task2 = Task::new(
        "2".to_string(),
        "Create data models".to_string(),
        "Define structs for domain".to_string(),
    );
    task2.dependencies = vec!["1".to_string()];

    // Task 3: Depends on tasks 1 and 2, pending
    let mut task3 = Task::new(
        "3".to_string(),
        "Implement API handlers".to_string(),
        "Create HTTP endpoints".to_string(),
    );
    task3.dependencies = vec!["1".to_string(), "2".to_string()];

    phase.add_task(task1);
    phase.add_task(task2);
    phase.add_task(task3);

    // Find the next task with all dependencies met
    if let Some(next_task) = phase.find_next_task() {
        println!(
            "Next available task: {} - {}",
            next_task.id, next_task.title
        );
        println!("  (Task 3 is blocked because task 2 is not done yet)\n");
    }

    // Check individual dependency status
    if let Some(task3) = phase.get_task("3") {
        let deps_met = task3.has_dependencies_met(&phase.tasks);
        println!("Task 3 dependencies met: {}", deps_met);
    }

    // Get actionable tasks (excludes expanded parents, includes ready subtasks)
    let actionable = phase.get_actionable_tasks();
    println!("Actionable tasks: {}\n", actionable.len());
}

/// Demonstrates loading and saving tasks using Storage
fn demonstrate_storage_operations() {
    println!("--- Storage Operations ---\n");

    // Create storage instance (uses current directory by default)
    let storage = Storage::new(None);

    // Check if SCUD is initialized in this directory
    if !storage.is_initialized() {
        println!("SCUD not initialized in current directory.");
        println!("Run 'scud init' to initialize, or specify a project root.\n");
        return;
    }

    // Load all phases (task groups)
    match storage.load_tasks() {
        Ok(phases) => {
            println!("Loaded {} phase(s):", phases.len());
            for (tag, phase) in &phases {
                let stats = phase.get_stats();
                println!("  {} - {} tasks ({} done)", tag, stats.total, stats.done);
            }
            println!();

            // Flatten all tasks for cross-phase dependency resolution
            let all_tasks: Vec<&Task> = phases.values().flat_map(|p| p.tasks.iter()).collect();
            println!("Total tasks across all phases: {}", all_tasks.len());
        }
        Err(e) => {
            println!("Failed to load tasks: {}\n", e);
            return;
        }
    }

    // Load the active phase (currently selected tag)
    match storage.get_active_group() {
        Ok(Some(active_tag)) => {
            println!("Active tag: {}", active_tag);

            // Load just the active phase
            if let Ok(phase) = storage.load_group(&active_tag) {
                // Find next task with cross-phase dependency support
                // First, load all tasks for dependency resolution
                if let Ok(all_phases) = storage.load_tasks() {
                    let all_tasks: Vec<&Task> =
                        all_phases.values().flat_map(|p| p.tasks.iter()).collect();

                    if let Some(next) = phase.find_next_task_cross_tag(&all_tasks) {
                        println!(
                            "Next task in '{}': {} - {}",
                            active_tag, next.id, next.title
                        );
                    }
                }
            }
        }
        Ok(None) => {
            println!("No active tag set. Use 'scud tags --set <tag>' to set one.");
        }
        Err(e) => {
            println!("Error getting active group: {}", e);
        }
    }
    println!();
}

/// Demonstrates running backpressure validation
fn demonstrate_backpressure_validation() {
    println!("--- Backpressure Validation ---\n");

    // Load configuration (auto-detects project type if not configured)
    let config = match BackpressureConfig::load(None) {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("Failed to load backpressure config: {}", e);
            println!("Using default configuration.\n");
            BackpressureConfig::default()
        }
    };

    if config.commands.is_empty() {
        println!("No validation commands configured or detected.");
        println!("Auto-detection checks for: Cargo.toml, package.json, go.mod, etc.\n");

        // Create a custom config for demonstration
        let demo_config = BackpressureConfig {
            commands: vec!["cargo check".to_string(), "cargo test --no-run".to_string()],
            stop_on_failure: true,
            timeout_secs: 300,
        };

        println!("Example custom configuration:");
        println!("  Commands: {:?}", demo_config.commands);
        println!("  Stop on failure: {}", demo_config.stop_on_failure);
        println!("  Timeout: {}s\n", demo_config.timeout_secs);
        return;
    }

    println!("Detected validation commands:");
    for cmd in &config.commands {
        println!("  - {}", cmd);
    }
    println!();

    // Run validation (comment out to skip actual execution)
    println!("Running validation...\n");

    let working_dir = Path::new(".");
    match run_validation(working_dir, &config) {
        Ok(result) => {
            if result.all_passed {
                println!("All validation checks passed!");
            } else {
                println!("Validation failed:");
                for failure in &result.failures {
                    println!("  - {}", failure);
                }
            }

            // Print detailed results
            println!("\nDetailed results:");
            for cmd_result in &result.results {
                let status = if cmd_result.passed { "PASS" } else { "FAIL" };
                println!(
                    "  [{}] {} ({:.2}s)",
                    status, cmd_result.command, cmd_result.duration_secs
                );

                if !cmd_result.passed && !cmd_result.stderr.is_empty() {
                    // Print first few lines of stderr
                    let stderr_preview: String = cmd_result
                        .stderr
                        .lines()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("\n    ");
                    println!("    stderr: {}", stderr_preview);
                }
            }
        }
        Err(e) => {
            println!("Validation error: {}", e);
        }
    }
    println!();
}
