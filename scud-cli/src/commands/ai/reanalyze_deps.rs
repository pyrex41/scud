use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::llm::{LLMClient, Prompts};
use crate::models::{Phase, TaskStatus};
use crate::storage::Storage;

#[derive(Debug, Deserialize)]
struct DependencySuggestion {
    task_id: String,
    add_dependencies: Vec<String>,
    remove_dependencies: Vec<String>,
    reasoning: String,
}

pub async fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    all_tags: bool,
    apply: bool,
    dry_run: bool,
    model: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    let mut all_phases = storage.load_tasks()?;

    if all_phases.is_empty() {
        println!(
            "{}",
            "No tasks found. Create tasks first with: scud parse-prd".yellow()
        );
        return Ok(());
    }

    // Determine which phases to analyze
    let phases_to_analyze: Vec<String> = match tag {
        Some(t) if !all_tags => {
            if !all_phases.contains_key(t) {
                anyhow::bail!("Tag '{}' not found", t);
            }
            vec![t.to_string()]
        }
        _ => all_phases.keys().cloned().collect(),
    };

    if phases_to_analyze.len() == 1 && all_phases.len() == 1 {
        println!(
            "{}",
            "Only one phase exists. Cross-tag dependency analysis is most useful with multiple phases.".yellow()
        );
    }

    // Build context for AI: all tasks with their current state
    let task_context = build_task_context(&all_phases);

    // Create LLM client
    let client = match project_root {
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
    spinner.set_message(format!(
        "Analyzing dependencies across {} phase(s)...",
        phases_to_analyze.len()
    ));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    // Generate analysis prompt and call LLM
    let prompt = Prompts::reanalyze_dependencies(&task_context, &phases_to_analyze);
    let suggestions: Vec<DependencySuggestion> = client.complete_json_with_model(&prompt, model).await?;

    spinner.finish_and_clear();

    // Filter out suggestions that don't change anything
    let meaningful_suggestions: Vec<_> = suggestions
        .into_iter()
        .filter(|s| !s.add_dependencies.is_empty() || !s.remove_dependencies.is_empty())
        .collect();

    if meaningful_suggestions.is_empty() {
        println!("{} No dependency changes suggested.", "✓".green());
        return Ok(());
    }

    // Display suggestions
    println!(
        "\n{} {} dependency change(s) suggested:\n",
        "Found".blue(),
        meaningful_suggestions.len()
    );

    for suggestion in &meaningful_suggestions {
        println!("{} {}", "Task:".bold(), suggestion.task_id.cyan());
        if !suggestion.add_dependencies.is_empty() {
            println!(
                "  {} {}",
                "+".green(),
                suggestion
                    .add_dependencies
                    .iter()
                    .map(|s| s.green().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if !suggestion.remove_dependencies.is_empty() {
            println!(
                "  {} {}",
                "-".red(),
                suggestion
                    .remove_dependencies
                    .iter()
                    .map(|s| s.red().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        println!("  {} {}", "Reason:".dimmed(), suggestion.reasoning.dimmed());
        println!();
    }

    if dry_run {
        println!("{}", "Dry run - no changes applied.".yellow());
        return Ok(());
    }

    // Apply changes
    let should_apply = apply || confirm_apply()?;

    if should_apply {
        let changes = apply_suggestions(&mut all_phases, &meaningful_suggestions)?;
        storage.save_tasks(&all_phases)?;
        println!(
            "{} {} dependencies updated across {} task(s).",
            "✓".green(),
            changes,
            meaningful_suggestions.len()
        );
    } else {
        println!("{}", "No changes applied.".yellow());
    }

    Ok(())
}

fn build_task_context(all_phases: &HashMap<String, Phase>) -> String {
    let mut context = String::new();

    for (tag, phase) in all_phases {
        context.push_str(&format!("\n## Phase: {}\n", tag));
        for task in &phase.tasks {
            let status_marker = match task.status {
                TaskStatus::Done => "[DONE]",
                TaskStatus::InProgress => "[IN PROGRESS]",
                TaskStatus::Pending => "[PENDING]",
                TaskStatus::Blocked => "[BLOCKED]",
                TaskStatus::Expanded => "[EXPANDED]",
                TaskStatus::Review => "[REVIEW]",
                TaskStatus::Deferred => "[DEFERRED]",
                TaskStatus::Cancelled => "[CANCELLED]",
            };

            // Build full namespaced ID for display
            let full_id = if task.id.contains(':') {
                task.id.clone()
            } else {
                format!("{}:{}", tag, task.id)
            };

            let deps_str = if task.dependencies.is_empty() {
                "none".to_string()
            } else {
                task.dependencies.join(", ")
            };

            context.push_str(&format!(
                "- {} {} - {}\n  Current deps: [{}]\n",
                full_id, status_marker, task.title, deps_str
            ));
        }
    }

    context
}

fn confirm_apply() -> Result<bool> {
    use std::io::{self, Write};

    print!("Apply these changes? [y/N] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes"))
}

fn apply_suggestions(
    all_phases: &mut HashMap<String, Phase>,
    suggestions: &[DependencySuggestion],
) -> Result<usize> {
    let mut changes = 0;

    for suggestion in suggestions {
        // Parse task ID to find the phase
        let (phase_tag, local_id) = parse_task_id(&suggestion.task_id);

        // Find the phase
        let phase = all_phases.get_mut(&phase_tag).ok_or_else(|| {
            anyhow::anyhow!(
                "Phase '{}' not found for task '{}'",
                phase_tag,
                suggestion.task_id
            )
        })?;

        // Find the task - try both local ID and full ID
        let task = phase
            .tasks
            .iter_mut()
            .find(|t| t.id == local_id || t.id == suggestion.task_id)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Task '{}' not found in phase '{}'",
                    suggestion.task_id,
                    phase_tag
                )
            })?;

        // Add new dependencies
        for dep in &suggestion.add_dependencies {
            if !task.dependencies.contains(dep) {
                task.dependencies.push(dep.clone());
                changes += 1;
            }
        }

        // Remove dependencies
        for dep in &suggestion.remove_dependencies {
            if let Some(pos) = task.dependencies.iter().position(|d| d == dep) {
                task.dependencies.remove(pos);
                changes += 1;
            }
        }
    }

    Ok(changes)
}

/// Parse a task ID into (phase_tag, local_id)
/// Examples:
///   "auth:1" -> ("auth", "1")
///   "main:9.1" -> ("main", "9.1")
///   "1" -> ("", "1")  -- no phase prefix
fn parse_task_id(task_id: &str) -> (String, String) {
    if let Some(colon_pos) = task_id.find(':') {
        let phase = task_id[..colon_pos].to_string();
        let local = task_id[colon_pos + 1..].to_string();
        (phase, local)
    } else {
        // This shouldn't happen with our prompts, but handle gracefully
        (String::new(), task_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Task;

    #[test]
    fn test_parse_task_id_with_namespace() {
        let (phase, local) = parse_task_id("auth:1");
        assert_eq!(phase, "auth");
        assert_eq!(local, "1");
    }

    #[test]
    fn test_parse_task_id_with_subtask() {
        let (phase, local) = parse_task_id("main:9.1");
        assert_eq!(phase, "main");
        assert_eq!(local, "9.1");
    }

    #[test]
    fn test_parse_task_id_with_nested_subtask() {
        let (phase, local) = parse_task_id("api:2.3.1");
        assert_eq!(phase, "api");
        assert_eq!(local, "2.3.1");
    }

    #[test]
    fn test_parse_task_id_without_namespace() {
        let (phase, local) = parse_task_id("1");
        assert_eq!(phase, "");
        assert_eq!(local, "1");
    }

    #[test]
    fn test_build_task_context() {
        let mut phases = HashMap::new();

        let mut auth_phase = Phase::new("auth".to_string());
        let mut task1 = Task::new(
            "auth:1".to_string(),
            "Create user model".to_string(),
            "".to_string(),
        );
        task1.status = TaskStatus::Done;
        auth_phase.add_task(task1);

        let mut api_phase = Phase::new("api".to_string());
        let mut task2 = Task::new(
            "api:1".to_string(),
            "Create endpoints".to_string(),
            "".to_string(),
        );
        task2.dependencies = vec!["auth:1".to_string()];
        api_phase.add_task(task2);

        phases.insert("auth".to_string(), auth_phase);
        phases.insert("api".to_string(), api_phase);

        let context = build_task_context(&phases);

        assert!(context.contains("Phase: auth"));
        assert!(context.contains("Phase: api"));
        assert!(context.contains("[DONE]"));
        assert!(context.contains("auth:1"));
    }
}
