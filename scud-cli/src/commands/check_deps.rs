use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::models::{Phase, TaskStatus};
use crate::storage::Storage;

/// Results from dependency validation
#[derive(Debug, Default)]
pub struct DepCheckResults {
    pub missing_deps: Vec<(String, String, String)>,      // (tag, task_id, missing_dep)
    pub invalid_zero_deps: Vec<(String, String)>,          // (tag, task_id)
    pub self_refs: Vec<(String, String)>,                  // (tag, task_id)
    pub cancelled_deps: Vec<(String, String, String)>,     // (tag, task_id, cancelled_dep)
}

impl DepCheckResults {
    pub fn has_issues(&self) -> bool {
        !self.missing_deps.is_empty()
            || !self.invalid_zero_deps.is_empty()
            || !self.self_refs.is_empty()
            || !self.cancelled_deps.is_empty()
    }

    pub fn issue_count(&self) -> usize {
        self.missing_deps.len()
            + self.invalid_zero_deps.len()
            + self.self_refs.len()
            + self.cancelled_deps.len()
    }
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    all_tags: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    let all_phases = storage.load_tasks()?;

    if all_phases.is_empty() {
        println!("{}", "No tasks found.".yellow());
        return Ok(());
    }

    // Determine which phases to check
    let phases_to_check: Vec<String> = match tag {
        Some(t) if !all_tags => {
            if !all_phases.contains_key(t) {
                anyhow::bail!("Tag '{}' not found", t);
            }
            vec![t.to_string()]
        }
        _ => all_phases.keys().cloned().collect(),
    };

    println!(
        "{} Checking dependencies across {} phase(s)...\n",
        "Validating".blue(),
        phases_to_check.len()
    );

    let mut results = DepCheckResults::default();

    // Build global task ID set for cross-phase validation
    let all_task_ids: HashSet<String> = all_phases
        .iter()
        .flat_map(|(tag, phase)| {
            phase.tasks.iter().flat_map(move |t| {
                let mut ids = vec![t.id.clone(), format!("{}:{}", tag, t.id)];
                // Also add subtask IDs if present
                for subtask_id in &t.subtasks {
                    ids.push(subtask_id.clone());
                    ids.push(format!("{}:{}", tag, subtask_id));
                }
                ids
            })
        })
        .collect();

    // Validate each phase
    for tag in &phases_to_check {
        if let Some(phase) = all_phases.get(tag) {
            validate_phase(tag, phase, &all_task_ids, &mut results);
        }
    }

    // Print results
    print_results(&results);

    if results.has_issues() {
        std::process::exit(1);
    }

    Ok(())
}

fn validate_phase(
    tag: &str,
    phase: &Phase,
    all_task_ids: &HashSet<String>,
    results: &mut DepCheckResults,
) {
    let local_ids: HashSet<_> = phase.tasks.iter().map(|t| t.id.clone()).collect();

    for task in &phase.tasks {
        // Skip completed/cancelled tasks
        if matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) {
            continue;
        }

        for dep in &task.dependencies {
            // Check for invalid "0" reference
            if dep == "0" || dep.ends_with(":0") {
                results.invalid_zero_deps.push((tag.to_string(), task.id.clone()));
                continue;
            }

            // Check for self-reference
            if dep == &task.id || dep == &format!("{}:{}", tag, task.id) {
                results.self_refs.push((tag.to_string(), task.id.clone()));
                continue;
            }

            // Check if dependency exists
            let exists = local_ids.contains(dep)
                || all_task_ids.contains(dep)
                || all_task_ids.contains(&format!("{}:{}", tag, dep));

            if !exists {
                results.missing_deps.push((
                    tag.to_string(),
                    task.id.clone(),
                    dep.clone(),
                ));
                continue;
            }

            // Check if dependency is cancelled
            if let Some(dep_task) = phase.get_task(dep) {
                if dep_task.status == TaskStatus::Cancelled {
                    results.cancelled_deps.push((
                        tag.to_string(),
                        task.id.clone(),
                        dep.clone(),
                    ));
                }
            }
        }
    }
}

fn print_results(results: &DepCheckResults) {
    if !results.has_issues() {
        println!("{}", "✓ No dependency issues found!".green().bold());
        return;
    }

    // Invalid zero references
    if !results.invalid_zero_deps.is_empty() {
        println!("{}", "Invalid Task Zero References".red().bold());
        println!("{}", "-".repeat(40).red());
        for (tag, task_id) in &results.invalid_zero_deps {
            println!(
                "  {} Task {} references invalid task \"0\"",
                "✗".red(),
                format!("{}:{}", tag, task_id).cyan()
            );
            println!(
                "    {}",
                "→ Task indices start at 1. Remove or update this dependency.".dimmed()
            );
        }
        println!();
    }

    // Missing dependencies
    if !results.missing_deps.is_empty() {
        println!("{}", "Missing Dependencies".red().bold());
        println!("{}", "-".repeat(40).red());
        for (tag, task_id, dep) in &results.missing_deps {
            println!(
                "  {} Task {} depends on non-existent task {}",
                "✗".red(),
                format!("{}:{}", tag, task_id).cyan(),
                dep.yellow()
            );
            println!(
                "    {}",
                format!("→ Remove dependency or create task {}", dep).dimmed()
            );
        }
        println!();
    }

    // Self-references
    if !results.self_refs.is_empty() {
        println!("{}", "Self-Referencing Dependencies".red().bold());
        println!("{}", "-".repeat(40).red());
        for (tag, task_id) in &results.self_refs {
            println!(
                "  {} Task {} depends on itself",
                "✗".red(),
                format!("{}:{}", tag, task_id).cyan()
            );
            println!("    {}", "→ Remove self-referencing dependency.".dimmed());
        }
        println!();
    }

    // Cancelled dependencies
    if !results.cancelled_deps.is_empty() {
        println!("{}", "Dependencies on Cancelled Tasks".yellow().bold());
        println!("{}", "-".repeat(40).yellow());
        for (tag, task_id, dep) in &results.cancelled_deps {
            println!(
                "  {} Task {} depends on cancelled task {}",
                "⚠".yellow(),
                format!("{}:{}", tag, task_id).cyan(),
                dep.yellow()
            );
            println!(
                "    {}",
                format!("→ Remove dependency or un-cancel {}", dep).dimmed()
            );
        }
        println!();
    }

    // Summary
    println!("{}", "Summary".blue().bold());
    println!("{}", "-".repeat(40).blue());
    println!(
        "  Total issues: {}",
        results.issue_count().to_string().red()
    );
    println!();
    println!("{}", "To fix issues:".blue());
    println!("  - Edit .scud/<tag>.scg directly");
    println!("  - Or run: scud reanalyze-deps --apply");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Task;

    #[test]
    fn test_results_has_issues() {
        let mut results = DepCheckResults::default();
        assert!(!results.has_issues());

        results.missing_deps.push(("test".to_string(), "1".to_string(), "99".to_string()));
        assert!(results.has_issues());
    }

    #[test]
    fn test_detect_invalid_zero() {
        let mut phase = Phase::new("test".to_string());
        let mut task = Task::new("1".to_string(), "Test".to_string(), "".to_string());
        task.dependencies = vec!["0".to_string()];
        phase.add_task(task);

        let all_ids: HashSet<String> = ["1".to_string()].into_iter().collect();
        let mut results = DepCheckResults::default();

        validate_phase("test", &phase, &all_ids, &mut results);

        assert_eq!(results.invalid_zero_deps.len(), 1);
        assert_eq!(results.invalid_zero_deps[0], ("test".to_string(), "1".to_string()));
    }

    #[test]
    fn test_detect_missing_dep() {
        let mut phase = Phase::new("test".to_string());
        let mut task = Task::new("1".to_string(), "Test".to_string(), "".to_string());
        task.dependencies = vec!["99".to_string()];
        phase.add_task(task);

        let all_ids: HashSet<String> = ["1".to_string()].into_iter().collect();
        let mut results = DepCheckResults::default();

        validate_phase("test", &phase, &all_ids, &mut results);

        assert_eq!(results.missing_deps.len(), 1);
    }

    #[test]
    fn test_valid_deps_no_issues() {
        let mut phase = Phase::new("test".to_string());

        let task1 = Task::new("1".to_string(), "First".to_string(), "".to_string());
        let mut task2 = Task::new("2".to_string(), "Second".to_string(), "".to_string());
        task2.dependencies = vec!["1".to_string()];

        phase.add_task(task1);
        phase.add_task(task2);

        let all_ids: HashSet<String> = ["1".to_string(), "2".to_string()].into_iter().collect();
        let mut results = DepCheckResults::default();

        validate_phase("test", &phase, &all_ids, &mut results);

        assert!(!results.has_issues());
    }
}
