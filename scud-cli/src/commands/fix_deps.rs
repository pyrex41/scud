use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::llm::{LLMClient, Prompts};
use crate::models::{Phase, TaskStatus};
use crate::storage::Storage;

use super::check_deps::{validate_phase, DepCheckResults};

/// Suggested fix from AI analysis
#[derive(Debug, Deserialize)]
pub struct DepFix {
    pub task_id: String,
    #[serde(default)]
    pub add_dependencies: Vec<String>,
    #[serde(default)]
    pub remove_dependencies: Vec<String>,
    pub reasoning: String,
}

pub async fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    all_tags: bool,
    dry_run: bool,
    model: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    let mut all_phases = storage.load_tasks()?;

    if all_phases.is_empty() {
        println!("{}", "No tasks found.".yellow());
        return Ok(());
    }

    // Determine which phases to fix
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
        "{} Analyzing dependencies across {} phase(s)...\n",
        "Analyzing".blue(),
        phases_to_check.len()
    );

    // Build global task ID set for cross-phase validation
    let all_task_ids: HashSet<String> = all_phases
        .iter()
        .flat_map(|(tag, phase)| {
            phase.tasks.iter().flat_map(move |t| {
                let mut ids = vec![t.id.clone(), format!("{}:{}", tag, t.id)];
                for subtask_id in &t.subtasks {
                    ids.push(subtask_id.clone());
                    ids.push(format!("{}:{}", tag, subtask_id));
                }
                ids
            })
        })
        .collect();

    // First, run check_deps validation to identify issues
    let mut results = DepCheckResults::default();
    for tag in &phases_to_check {
        if let Some(phase) = all_phases.get(tag) {
            validate_phase(tag, phase, &all_task_ids, &mut results);
        }
    }

    if !results.has_issues() {
        println!("{}", "✓ No dependency issues found!".green().bold());
        return Ok(());
    }

    println!(
        "{} Found {} issue(s) to fix\n",
        "Found".yellow(),
        results.issue_count()
    );

    // Track fixes applied
    let mut fixes_applied: HashMap<String, Vec<String>> = HashMap::new();

    // Phase 1: Apply automatic fixes (removals)
    println!("{}", "Phase 1: Automatic Fixes".blue().bold());
    println!("{}", "-".repeat(40).blue());

    // Fix invalid zero references
    for (tag, task_id) in &results.invalid_zero_deps {
        let full_id = format!("{}:{}", tag, task_id);
        if let Some(phase) = all_phases.get_mut(tag) {
            if let Some(task) = phase.tasks.iter_mut().find(|t| &t.id == task_id) {
                let before_len = task.dependencies.len();
                task.dependencies.retain(|d| d != "0" && !d.ends_with(":0"));
                let removed = before_len - task.dependencies.len();
                if removed > 0 {
                    let msg = format!("Removed {} invalid '0' reference(s)", removed);
                    println!("  {} {} - {}", "✓".green(), full_id.cyan(), msg);
                    fixes_applied
                        .entry(full_id.clone())
                        .or_default()
                        .push(msg);
                }
            }
        }
    }

    // Fix self-references
    for (tag, task_id) in &results.self_refs {
        let full_id = format!("{}:{}", tag, task_id);
        if let Some(phase) = all_phases.get_mut(tag) {
            if let Some(task) = phase.tasks.iter_mut().find(|t| &t.id == task_id) {
                let self_ref = task_id.clone();
                let self_ref_full = format!("{}:{}", tag, task_id);
                let before_len = task.dependencies.len();
                task.dependencies
                    .retain(|d| d != &self_ref && d != &self_ref_full);
                let removed = before_len - task.dependencies.len();
                if removed > 0 {
                    let msg = "Removed self-reference".to_string();
                    println!("  {} {} - {}", "✓".green(), full_id.cyan(), msg);
                    fixes_applied
                        .entry(full_id.clone())
                        .or_default()
                        .push(msg);
                }
            }
        }
    }

    // Fix missing dependencies (remove them)
    for (tag, task_id, missing_dep) in &results.missing_deps {
        let full_id = format!("{}:{}", tag, task_id);
        if let Some(phase) = all_phases.get_mut(tag) {
            if let Some(task) = phase.tasks.iter_mut().find(|t| &t.id == task_id) {
                let before_len = task.dependencies.len();
                task.dependencies.retain(|d| d != missing_dep);
                let removed = before_len - task.dependencies.len();
                if removed > 0 {
                    let msg = format!("Removed non-existent dependency '{}'", missing_dep);
                    println!("  {} {} - {}", "✓".green(), full_id.cyan(), msg);
                    fixes_applied
                        .entry(full_id.clone())
                        .or_default()
                        .push(msg);
                }
            }
        }
    }

    // Fix cancelled dependencies (remove them)
    for (tag, task_id, cancelled_dep) in &results.cancelled_deps {
        let full_id = format!("{}:{}", tag, task_id);
        if let Some(phase) = all_phases.get_mut(tag) {
            if let Some(task) = phase.tasks.iter_mut().find(|t| &t.id == task_id) {
                let before_len = task.dependencies.len();
                task.dependencies.retain(|d| d != cancelled_dep);
                let removed = before_len - task.dependencies.len();
                if removed > 0 {
                    let msg = format!("Removed dependency on cancelled task '{}'", cancelled_dep);
                    println!("  {} {} - {}", "✓".green(), full_id.cyan(), msg);
                    fixes_applied
                        .entry(full_id.clone())
                        .or_default()
                        .push(msg);
                }
            }
        }
    }

    println!();

    // Phase 2: AI-powered dependency suggestions
    println!("{}", "Phase 2: AI Dependency Analysis".blue().bold());
    println!("{}", "-".repeat(40).blue());

    // Build task context for AI
    let task_context = build_task_context(&all_phases, &phases_to_check);

    // Create LLM client
    let client = match project_root.clone() {
        Some(root) => LLMClient::new_with_project_root(root)?,
        None => LLMClient::new()?,
    };

    // Show progress
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    spinner.set_message("Analyzing dependencies with AI...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    // Call LLM to suggest fixes (use smart model for analysis tasks)
    let prompt = Prompts::reanalyze_dependencies(&task_context, &phases_to_check);
    let suggestions: Vec<DepFix> = client.complete_json_smart(&prompt, model).await?;

    spinner.finish_and_clear();

    if suggestions.is_empty() {
        println!("  {} No additional dependency changes suggested by AI", "ℹ".blue());
    } else {
        println!(
            "  {} AI suggested {} change(s):\n",
            "ℹ".blue(),
            suggestions.len()
        );

        for suggestion in &suggestions {
            // Parse task_id to get tag and id
            let (suggestion_tag, suggestion_task_id) = if suggestion.task_id.contains(':') {
                let parts: Vec<&str> = suggestion.task_id.split(':').collect();
                (parts[0].to_string(), parts[1].to_string())
            } else {
                // Assume current tag if not specified
                let tag = phases_to_check.first().cloned().unwrap_or_default();
                (tag, suggestion.task_id.clone())
            };

            println!("  {} {}", "→".cyan(), suggestion.task_id.cyan().bold());

            if !suggestion.add_dependencies.is_empty() {
                println!(
                    "    {} {}",
                    "Add:".green(),
                    suggestion.add_dependencies.join(", ").green()
                );
            }

            if !suggestion.remove_dependencies.is_empty() {
                println!(
                    "    {} {}",
                    "Remove:".red(),
                    suggestion.remove_dependencies.join(", ").red()
                );
            }

            println!("    {} {}", "Reason:".dimmed(), suggestion.reasoning.dimmed());

            // Apply AI suggestions
            if let Some(phase) = all_phases.get_mut(&suggestion_tag) {
                if let Some(task) = phase
                    .tasks
                    .iter_mut()
                    .find(|t| t.id == suggestion_task_id)
                {
                    // Skip completed/cancelled tasks
                    if matches!(task.status, TaskStatus::Done | TaskStatus::Cancelled) {
                        println!(
                            "    {} Task is {} - skipping",
                            "⚠".yellow(),
                            format!("{:?}", task.status).yellow()
                        );
                        continue;
                    }

                    let mut changes = Vec::new();

                    // Add new dependencies
                    for dep in &suggestion.add_dependencies {
                        // Validate the dependency exists and isn't task 0
                        if dep == "0" || dep.ends_with(":0") {
                            println!(
                                "    {} Skipping invalid '0' dependency suggestion",
                                "⚠".yellow()
                            );
                            continue;
                        }

                        if !task.dependencies.contains(dep) && all_task_ids.contains(dep) {
                            task.dependencies.push(dep.clone());
                            changes.push(format!("Added dependency on '{}'", dep));
                        }
                    }

                    // Remove dependencies
                    for dep in &suggestion.remove_dependencies {
                        if task.dependencies.contains(dep) {
                            task.dependencies.retain(|d| d != dep);
                            changes.push(format!("Removed dependency on '{}'", dep));
                        }
                    }

                    if !changes.is_empty() {
                        let full_id = format!("{}:{}", suggestion_tag, suggestion_task_id);
                        fixes_applied
                            .entry(full_id)
                            .or_default()
                            .extend(changes);
                    }
                }
            }

            println!();
        }
    }

    // Summary
    println!();
    println!("{}", "Fix Summary".blue().bold());
    println!("{}", "-".repeat(40).blue());

    let total_tasks_fixed = fixes_applied.len();
    let total_changes: usize = fixes_applied.values().map(|v| v.len()).sum();

    if total_changes == 0 {
        println!("  No changes needed.");
    } else {
        println!(
            "  {} task(s) with {} total change(s)",
            total_tasks_fixed.to_string().green(),
            total_changes.to_string().green()
        );

        if dry_run {
            println!();
            println!(
                "{}",
                "DRY RUN - no changes saved. Remove --dry-run to apply.".yellow()
            );
        } else {
            // Save changes
            storage.save_tasks(&all_phases)?;
            println!();
            println!("{}", "✓ Changes saved successfully!".green().bold());
        }
    }

    println!();
    println!("{}", "Next steps:".blue());
    println!("  1. Review changes: scud list -v");
    println!("  2. Validate: scud check-deps");
    println!("  3. View dependency graph: scud mermaid");

    Ok(())
}

fn build_task_context(
    all_phases: &HashMap<String, Phase>,
    phases_to_check: &[String],
) -> String {
    let mut context = String::new();

    for tag in phases_to_check {
        if let Some(phase) = all_phases.get(tag) {
            context.push_str(&format!("## Phase: {}\n\n", tag));

            for task in &phase.tasks {
                let deps_str = if task.dependencies.is_empty() {
                    "none".to_string()
                } else {
                    task.dependencies.join(", ")
                };

                context.push_str(&format!(
                    "- {}:{} [{}] - {}\n  Dependencies: {}\n",
                    tag,
                    task.id,
                    format!("{:?}", task.status),
                    task.title,
                    deps_str
                ));
            }

            context.push('\n');
        }
    }

    context
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Task;

    #[test]
    fn test_build_task_context() {
        let mut phase = Phase::new("test".to_string());
        let mut task1 = Task::new("1".to_string(), "First task".to_string(), "".to_string());
        task1.dependencies = vec![];
        let mut task2 = Task::new("2".to_string(), "Second task".to_string(), "".to_string());
        task2.dependencies = vec!["1".to_string()];

        phase.add_task(task1);
        phase.add_task(task2);

        let mut phases = HashMap::new();
        phases.insert("test".to_string(), phase);

        let context = build_task_context(&phases, &["test".to_string()]);

        assert!(context.contains("## Phase: test"));
        assert!(context.contains("test:1"));
        assert!(context.contains("test:2"));
        assert!(context.contains("Dependencies: 1"));
    }
}
