use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::path::PathBuf;

use crate::models::task::TaskStatus;
use crate::storage::Storage;

/// Diagnostic issue severity
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Warning => "WARNING",
            Severity::Error => "ERROR",
            Severity::Critical => "CRITICAL",
        }
    }
}

/// A diagnostic issue found by the doctor command
#[derive(Debug, Clone)]
pub struct DiagnosticIssue {
    pub severity: Severity,
    pub epic_tag: String,
    pub task_id: Option<String>,
    pub message: String,
    pub suggestion: String,
}

/// Results from running diagnostics
#[derive(Debug, Default)]
pub struct DiagnosticResults {
    pub issues: Vec<DiagnosticIssue>,
    pub stale_locks: Vec<(String, String, String, f64)>, // (epic, task_id, locked_by, hours)
    pub blocked_by_cancelled: Vec<(String, String, String)>, // (epic, task_id, blocked_dep)
    pub blocked_by_missing: Vec<(String, String, String)>, // (epic, task_id, missing_dep)
    pub orphan_in_progress: Vec<(String, String)>, // (epic, task_id) - in-progress >24h without lock
    pub missing_active_epic: bool,
    pub corrupt_files: Vec<String>,
}

impl DiagnosticResults {
    pub fn has_issues(&self) -> bool {
        !self.issues.is_empty()
            || !self.stale_locks.is_empty()
            || !self.blocked_by_cancelled.is_empty()
            || !self.blocked_by_missing.is_empty()
            || !self.orphan_in_progress.is_empty()
            || self.missing_active_epic
            || !self.corrupt_files.is_empty()
    }

    pub fn critical_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Critical)
            .count()
            + self.corrupt_files.len()
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
            + self.blocked_by_cancelled.len()
            + self.blocked_by_missing.len()
    }

    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
            + self.stale_locks.len()
            + self.orphan_in_progress.len()
            + if self.missing_active_epic { 1 } else { 0 }
    }
}

pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    stale_hours: f64,
    fix: bool,
) -> Result<()> {
    println!(
        "{}",
        "[EXPERIMENTAL] SCUD Doctor - Workflow Diagnostics"
            .blue()
            .bold()
    );
    println!("{}", "=".repeat(60).blue());
    println!();

    let storage = Storage::new(project_root);

    // Check if storage files exist and are readable
    let tasks_result = storage.load_tasks();
    let workflow_result = storage.load_workflow_state();

    let mut results = DiagnosticResults::default();

    // Check for corrupt/missing files
    if let Err(ref e) = tasks_result {
        results.corrupt_files.push(format!("tasks file: {}", e));
    }

    if let Err(ref e) = workflow_result {
        if !e.to_string().contains("not found") {
            results
                .corrupt_files
                .push(format!("workflow-state.json: {}", e));
        }
    }

    // Check active epic
    match storage.get_active_group() {
        Ok(Some(_)) => {}
        Ok(None) => {
            results.missing_active_epic = true;
        }
        Err(_) => {
            results.missing_active_epic = true;
        }
    }

    // If we couldn't load tasks, show what we found and exit
    if results.corrupt_files.len() > 0 {
        print_results(&results, fix);
        return Ok(());
    }

    let mut all_tasks = tasks_result?;

    // Filter to specific tag if provided
    let epic_tags: Vec<String> = if let Some(t) = tag {
        if all_tasks.contains_key(t) {
            vec![t.to_string()]
        } else {
            anyhow::bail!("Epic '{}' not found", t);
        }
    } else {
        all_tasks.keys().cloned().collect()
    };

    // Run diagnostics on each epic
    for epic_tag in &epic_tags {
        let epic = match all_tasks.get(epic_tag) {
            Some(e) => e,
            None => continue,
        };

        // Build set of all task IDs for dependency checking
        let all_task_ids: HashSet<_> = epic.tasks.iter().map(|t| t.id.clone()).collect();

        for task in &epic.tasks {
            // Check for stale locks
            if task.is_stale_lock(stale_hours) {
                if let (Some(locked_by), Some(hours)) = (&task.locked_by, task.lock_age_hours()) {
                    results.stale_locks.push((
                        epic_tag.clone(),
                        task.id.clone(),
                        locked_by.clone(),
                        hours,
                    ));
                }
            }

            // Check for orphan in-progress tasks (in-progress but not locked, >24h old)
            if task.status == TaskStatus::InProgress && !task.is_locked() {
                if let Some(ref updated_at) = task.updated_at {
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(updated_at) {
                        let hours =
                            (chrono::Utc::now().signed_duration_since(dt)).num_hours() as f64;
                        if hours > stale_hours {
                            results
                                .orphan_in_progress
                                .push((epic_tag.clone(), task.id.clone()));
                        }
                    }
                }
            }

            // Check for dependencies on cancelled/blocked tasks
            if task.status == TaskStatus::Pending {
                for dep_id in &task.dependencies {
                    // Check if dependency exists
                    if !all_task_ids.contains(dep_id) {
                        results.blocked_by_missing.push((
                            epic_tag.clone(),
                            task.id.clone(),
                            dep_id.clone(),
                        ));
                        continue;
                    }

                    // Check if dependency is cancelled or blocked
                    if let Some(dep_task) = epic.get_task(dep_id) {
                        match dep_task.status {
                            TaskStatus::Cancelled => {
                                results.blocked_by_cancelled.push((
                                    epic_tag.clone(),
                                    task.id.clone(),
                                    dep_id.clone(),
                                ));
                            }
                            TaskStatus::Blocked => {
                                results.issues.push(DiagnosticIssue {
                                    severity: Severity::Warning,
                                    epic_tag: epic_tag.clone(),
                                    task_id: Some(task.id.clone()),
                                    message: format!(
                                        "Task {} depends on blocked task {}",
                                        task.id, dep_id
                                    ),
                                    suggestion: format!(
                                        "Resolve blocker for {} or remove dependency",
                                        dep_id
                                    ),
                                });
                            }
                            TaskStatus::Deferred => {
                                results.issues.push(DiagnosticIssue {
                                    severity: Severity::Warning,
                                    epic_tag: epic_tag.clone(),
                                    task_id: Some(task.id.clone()),
                                    message: format!(
                                        "Task {} depends on deferred task {}",
                                        task.id, dep_id
                                    ),
                                    suggestion: format!("Un-defer {} or update dependency", dep_id),
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    // Apply fixes if requested
    if fix && results.has_issues() {
        println!("{}", "Attempting auto-fixes...".yellow());
        println!();

        let mut fixed_count = 0;

        // Fix stale locks
        for (epic_tag, task_id, locked_by, hours) in &results.stale_locks {
            if let Some(epic) = all_tasks.get_mut(epic_tag) {
                if let Some(task) = epic.get_task_mut(task_id) {
                    task.release();
                    println!(
                        "{} Released stale lock: {} (was locked by {} for {:.1}h)",
                        "✓".green(),
                        task_id.cyan(),
                        locked_by,
                        hours
                    );
                    fixed_count += 1;
                }
            }
        }

        // Fix orphan in-progress tasks (reset to pending)
        for (epic_tag, task_id) in &results.orphan_in_progress {
            if let Some(epic) = all_tasks.get_mut(epic_tag) {
                if let Some(task) = epic.get_task_mut(task_id) {
                    task.set_status(TaskStatus::Pending);
                    println!(
                        "{} Reset orphan task to pending: {}",
                        "✓".green(),
                        task_id.cyan()
                    );
                    fixed_count += 1;
                }
            }
        }

        if fixed_count > 0 {
            storage.save_tasks(&all_tasks)?;
            println!();
            println!("{} {} issue(s) fixed", "✓".green(), fixed_count);
        } else {
            println!(
                "{}",
                "No auto-fixable issues found. Manual intervention required.".yellow()
            );
        }
        println!();
    }

    print_results(&results, fix);

    Ok(())
}

fn print_results(results: &DiagnosticResults, fix_attempted: bool) {
    if !results.has_issues() {
        println!(
            "{}",
            "✓ No issues found! Workflow is healthy.".green().bold()
        );
        return;
    }

    // Print critical issues (corrupt files)
    if !results.corrupt_files.is_empty() {
        println!("{}", "CRITICAL: File Issues".red().bold());
        println!("{}", "-".repeat(40).red());
        for file_issue in &results.corrupt_files {
            println!("  {} {}", "✗".red(), file_issue);
        }
        println!();
        print_recovery_instructions();
        return;
    }

    // Print stale locks
    if !results.stale_locks.is_empty() {
        println!("{}", "Stale Locks (>threshold hours)".yellow().bold());
        println!("{}", "-".repeat(40).yellow());
        for (epic, task_id, locked_by, hours) in &results.stale_locks {
            println!(
                "  {} {} in {} locked by {} ({:.1}h)",
                "⚠".yellow(),
                task_id.cyan(),
                epic.dimmed(),
                locked_by.green(),
                hours
            );
            if !fix_attempted {
                println!(
                    "    {}",
                    format!("→ scud release {} --force -e {}", task_id, epic).dimmed()
                );
            }
        }
        println!();
    }

    // Print blocked by cancelled
    if !results.blocked_by_cancelled.is_empty() {
        println!("{}", "Tasks Blocked by Cancelled Dependencies".red().bold());
        println!("{}", "-".repeat(40).red());
        for (epic, task_id, dep_id) in &results.blocked_by_cancelled {
            println!(
                "  {} {} depends on cancelled task {}",
                "✗".red(),
                task_id.cyan(),
                dep_id.yellow()
            );
            println!(
                "    {}",
                format!(
                    "→ Remove dependency or un-cancel {} (in epic {})",
                    dep_id, epic
                )
                .dimmed()
            );
        }
        println!();
    }

    // Print blocked by missing
    if !results.blocked_by_missing.is_empty() {
        println!("{}", "Tasks with Missing Dependencies".red().bold());
        println!("{}", "-".repeat(40).red());
        for (epic, task_id, dep_id) in &results.blocked_by_missing {
            println!(
                "  {} {} depends on non-existent task {}",
                "✗".red(),
                task_id.cyan(),
                dep_id.yellow()
            );
            println!(
                "    {}",
                format!("→ Remove dependency from {} (in epic {})", task_id, epic).dimmed()
            );
        }
        println!();
    }

    // Print orphan in-progress
    if !results.orphan_in_progress.is_empty() {
        println!(
            "{}",
            "Orphan In-Progress Tasks (no lock, stale)".yellow().bold()
        );
        println!("{}", "-".repeat(40).yellow());
        for (epic, task_id) in &results.orphan_in_progress {
            println!(
                "  {} {} in {} - in-progress but not locked",
                "⚠".yellow(),
                task_id.cyan(),
                epic.dimmed()
            );
            if !fix_attempted {
                println!(
                    "    {}",
                    format!(
                        "→ scud set-status {} pending -t {}  # or done if complete",
                        task_id, epic
                    )
                    .dimmed()
                );
            }
        }
        println!();
    }

    // Print missing active epic
    if results.missing_active_epic {
        println!("{}", "No Active Epic Set".yellow().bold());
        println!("{}", "-".repeat(40).yellow());
        println!("  {} No active epic/tag is set", "⚠".yellow());
        println!(
            "    {}",
            "→ scud tags <epic-name>  # to set active epic".dimmed()
        );
        println!();
    }

    // Print other issues
    for issue in &results.issues {
        let (icon, color_fn): (&str, fn(&str) -> colored::ColoredString) = match issue.severity {
            Severity::Critical => ("✗", |s: &str| s.red()),
            Severity::Error => ("✗", |s: &str| s.red()),
            Severity::Warning => ("⚠", |s: &str| s.yellow()),
        };

        println!(
            "  {} [{}] {}",
            color_fn(icon),
            issue.severity.as_str(),
            issue.message
        );
        if let Some(ref task_id) = issue.task_id {
            println!(
                "    Task: {} in {}",
                task_id.cyan(),
                issue.epic_tag.dimmed()
            );
        }
        println!("    {}", format!("→ {}", issue.suggestion).dimmed());
    }

    // Summary
    println!();
    println!("{}", "Summary".blue().bold());
    println!("{}", "-".repeat(40).blue());
    println!(
        "  Critical: {}  Errors: {}  Warnings: {}",
        results.critical_count().to_string().red(),
        results.error_count().to_string().yellow(),
        results.warning_count().to_string().blue()
    );

    if !fix_attempted && (results.stale_locks.len() > 0 || results.orphan_in_progress.len() > 0) {
        println!();
        println!("{}", "To auto-fix recoverable issues, run:".blue());
        println!("  scud doctor --fix");
    }
}

fn print_recovery_instructions() {
    println!();
    println!("{}", "=".repeat(60).red());
    println!("{}", "RECOVERY INSTRUCTIONS".red().bold());
    println!("{}", "=".repeat(60).red());
    println!();
    println!("The task storage appears corrupted or missing. To recover:");
    println!();
    println!("1. Check if .scud/ directory exists:");
    println!("   {}", "ls -la .scud/".cyan());
    println!();
    println!("2. If missing, initialize SCUD:");
    println!("   {}", "scud init".cyan());
    println!();
    println!("3. If corrupted, check for backups:");
    println!("   {}", "ls -la .scud/tasks/*.bak".cyan());
    println!();
    println!("4. If no backups, you may need to recreate tasks:");
    println!(
        "   {}",
        "scud parse-prd <prd-file> --tag <epic-name>".cyan()
    );
    println!();
    println!("5. For manual recovery, task files are located at:");
    println!("   {}", ".scud/tasks/tasks.scg (or tasks.json)".dimmed());
    println!("   {}", ".scud/workflow-state.json".dimmed());
    println!();
    println!(
        "{}",
        "If issues persist, consider consulting a high-context agent".yellow()
    );
    println!(
        "{}",
        "with full codebase access to inspect and repair the files.".yellow()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::epic::Epic;
    use crate::models::task::Task;

    #[test]
    fn test_diagnostic_results_has_issues() {
        let empty = DiagnosticResults::default();
        assert!(!empty.has_issues());

        let mut with_stale = DiagnosticResults::default();
        with_stale.stale_locks.push((
            "epic".to_string(),
            "task".to_string(),
            "user".to_string(),
            48.0,
        ));
        assert!(with_stale.has_issues());
    }

    #[test]
    fn test_diagnostic_results_counts() {
        let mut results = DiagnosticResults::default();

        // Add stale locks (warnings)
        results.stale_locks.push((
            "epic".to_string(),
            "task1".to_string(),
            "user".to_string(),
            48.0,
        ));
        results.stale_locks.push((
            "epic".to_string(),
            "task2".to_string(),
            "user".to_string(),
            36.0,
        ));

        // Add blocked by cancelled (errors)
        results.blocked_by_cancelled.push((
            "epic".to_string(),
            "task3".to_string(),
            "dep1".to_string(),
        ));

        // Add corrupt files (critical)
        results
            .corrupt_files
            .push("tasks.json: parse error".to_string());

        assert_eq!(results.warning_count(), 2);
        assert_eq!(results.error_count(), 1);
        assert_eq!(results.critical_count(), 1);
    }

    #[test]
    fn test_severity_as_str() {
        assert_eq!(Severity::Warning.as_str(), "WARNING");
        assert_eq!(Severity::Error.as_str(), "ERROR");
        assert_eq!(Severity::Critical.as_str(), "CRITICAL");
    }

    fn create_test_epic_with_issues() -> Epic {
        let mut epic = Epic::new("test-epic".to_string());

        // Task 1: Done
        let mut task1 = Task::new("1".to_string(), "Task 1".to_string(), "Desc".to_string());
        task1.set_status(TaskStatus::Done);
        epic.add_task(task1);

        // Task 2: Cancelled (will block task 3)
        let mut task2 = Task::new("2".to_string(), "Task 2".to_string(), "Desc".to_string());
        task2.set_status(TaskStatus::Cancelled);
        epic.add_task(task2);

        // Task 3: Pending, depends on cancelled task 2
        let mut task3 = Task::new("3".to_string(), "Task 3".to_string(), "Desc".to_string());
        task3.dependencies = vec!["2".to_string()];
        epic.add_task(task3);

        // Task 4: Pending, depends on non-existent task
        let mut task4 = Task::new("4".to_string(), "Task 4".to_string(), "Desc".to_string());
        task4.dependencies = vec!["nonexistent".to_string()];
        epic.add_task(task4);

        epic
    }

    #[test]
    fn test_detect_cancelled_dependency() {
        let epic = create_test_epic_with_issues();

        let task3 = epic.get_task("3").unwrap();
        let mut found_cancelled_dep = false;

        for dep_id in &task3.dependencies {
            if let Some(dep_task) = epic.get_task(dep_id) {
                if dep_task.status == TaskStatus::Cancelled {
                    found_cancelled_dep = true;
                }
            }
        }

        assert!(found_cancelled_dep);
    }

    #[test]
    fn test_detect_missing_dependency() {
        let epic = create_test_epic_with_issues();
        let all_task_ids: std::collections::HashSet<_> =
            epic.tasks.iter().map(|t| t.id.clone()).collect();
        // Use all_task_ids to check for missing dependencies
        let _task_count = all_task_ids.len();

        let task4 = epic.get_task("4").unwrap();
        let mut found_missing_dep = false;

        for dep_id in &task4.dependencies {
            if !all_task_ids.contains(dep_id) {
                found_missing_dep = true;
            }
        }

        assert!(found_missing_dep);
    }

    #[test]
    fn test_stale_lock_detection() {
        let mut task = Task::new("1".to_string(), "Test".to_string(), "Desc".to_string());
        task.claim("alice").unwrap();

        // Not stale immediately
        assert!(!task.is_stale_lock(24.0));

        // Simulate old lock
        let two_days_ago = chrono::Utc::now() - chrono::Duration::hours(48);
        task.locked_at = Some(two_days_ago.to_rfc3339());

        assert!(task.is_stale_lock(24.0));
        assert!(!task.is_stale_lock(72.0));
    }
}
