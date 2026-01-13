use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::llm::{LLMClient, Prompts};
use crate::models::{IdFormat, Phase, Priority, Task};
use crate::storage::Storage;

#[derive(Debug, Deserialize)]
struct ParsedTask {
    title: String,
    description: String,
    priority: String,
    complexity: u32,
    #[serde(default)]
    dependencies: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub async fn run(
    project_root: Option<PathBuf>,
    file_path: &Path,
    tag: &str,
    num_tasks: u32,
    append: bool,
    no_guidance: bool,
    id_format: &str,
    model: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root.clone());

    if !storage.is_initialized() {
        anyhow::bail!("SCUD not initialized. Run: scud init");
    }

    // Read the PRD file
    println!("{} {}", "Reading PRD from:".blue(), file_path.display());
    let prd_content = storage.read_file(file_path)?;

    // Load guidance unless disabled
    let guidance = if no_guidance {
        None
    } else {
        match storage.load_guidance() {
            Ok(g) if !g.is_empty() => {
                println!("{}", "Loading project guidance...".blue());
                Some(g)
            }
            Ok(_) => None,
            Err(e) => {
                eprintln!("{} Failed to load guidance: {}", "Warning:".yellow(), e);
                None
            }
        }
    };

    // Create LLM client with proper project root
    let client = match project_root {
        Some(root) => LLMClient::new_with_project_root(root)?,
        None => LLMClient::new()?,
    };

    // Show model info
    let model_info = client.fast_model_info(model);
    println!("{} {}", "Using".blue(), model_info.to_string().cyan());

    // Show progress
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    spinner.set_message("Parsing PRD with AI...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    // Call LLM to parse the PRD (use fast model for generation tasks)
    let prompt = Prompts::parse_prd(&prd_content, num_tasks, guidance.as_deref());
    let parsed_tasks: Vec<ParsedTask> = client.complete_json_fast(&prompt, model).await?;

    spinner.finish_with_message(format!(
        "{} Parsed {} tasks",
        "✓".green(),
        parsed_tasks.len()
    ));

    // Load existing tasks (propagate errors - don't silently swallow them)
    let mut all_tasks = storage.load_tasks()?;

    // Check if other phases exist for cross-tag dependency hint
    let other_phases: Vec<_> = all_tasks.keys().filter(|k| *k != tag).cloned().collect();

    // Parse ID format from CLI argument
    let use_uuid = id_format == "uuid";
    let parsed_id_format = if use_uuid {
        IdFormat::Uuid
    } else {
        IdFormat::Sequential
    };

    // Determine starting ID based on append mode (only used for sequential format)
    let start_id = if append && all_tasks.contains_key(tag) {
        let existing = all_tasks.get(tag).unwrap();
        existing.tasks.len() + 1
    } else {
        1
    };

    // Convert to our task model
    let mut group = if append && all_tasks.contains_key(tag) {
        println!(
            "{}",
            format!("📎 Appending to existing task group '{}'...", tag).cyan()
        );
        let existing = all_tasks.get(tag).unwrap().clone();
        // When appending, warn if id_format doesn't match existing
        if existing.id_format != parsed_id_format {
            println!(
                "{}",
                format!(
                    "⚠ Ignoring --id-format: existing phase uses {} format",
                    existing.id_format.as_str()
                )
                .yellow()
            );
        }
        existing
    } else {
        if all_tasks.contains_key(tag) {
            println!(
                "{}",
                format!("⚠ Task group '{}' already exists. Replacing...", tag).yellow()
            );
        }
        let mut new_phase = Phase::new(tag.to_string());
        new_phase.id_format = parsed_id_format;
        new_phase
    };

    // Determine actual id_format to use (from phase, which may be inherited when appending)
    let use_uuid = group.id_format == IdFormat::Uuid;

    // Pre-generate all task IDs so we can remap dependencies
    let task_ids: Vec<String> = parsed_tasks
        .iter()
        .enumerate()
        .map(|(idx, _)| {
            if use_uuid {
                Uuid::new_v4().to_string().replace("-", "")
            } else {
                (start_id + idx).to_string()
            }
        })
        .collect();

    for (idx, parsed) in parsed_tasks.iter().enumerate() {
        let task_id = task_ids[idx].clone();

        let priority = match parsed.priority.to_lowercase().as_str() {
            "high" => Priority::High,
            "low" => Priority::Low,
            _ => Priority::Medium,
        };

        let mut task = Task::new(
            task_id.clone(),
            parsed.title.clone(),
            parsed.description.clone(),
        );
        task.complexity = parsed.complexity;
        task.priority = priority;

        // Map 1-indexed LLM dependency references to actual task IDs
        task.dependencies = parsed
            .dependencies
            .iter()
            .filter_map(|dep| {
                if let Ok(dep_idx) = dep.parse::<usize>() {
                    // Map 1-indexed reference to actual task ID
                    if dep_idx > 0 && dep_idx <= task_ids.len() {
                        Some(task_ids[dep_idx - 1].clone())
                    } else {
                        // Invalid index (0 or out of range) - skip
                        None
                    }
                } else {
                    // Already a full ID reference (cross-phase)
                    Some(dep.clone())
                }
            })
            .collect();

        group.add_task(task);
    }

    all_tasks.insert(tag.to_string(), group);
    storage.save_tasks(&all_tasks)?;

    // Set as active group
    storage.set_active_group(tag)?;

    println!(
        "\n{}",
        "✅ PRD parsed and task group created!".green().bold()
    );
    println!();
    println!("{:<20} {}", "Tag:".yellow(), tag.cyan());
    println!("{:<20} {}", "Tasks created:".yellow(), parsed_tasks.len());

    // Hint about cross-tag dependencies if other phases exist
    if !other_phases.is_empty() {
        println!();
        println!(
            "{} Other phases detected: {}",
            "Note:".cyan(),
            other_phases.join(", ").yellow()
        );
        println!(
            "      Consider running '{}' to identify cross-phase dependencies.",
            "scud reanalyze-deps".green()
        );
    }

    println!();
    println!("{}", "Next steps:".blue());
    println!("  1. Review tasks: scud list");
    println!("  2. Expand complex tasks: scud expand --all");
    if !other_phases.is_empty() {
        println!("  3. Check cross-phase deps: scud reanalyze-deps");
        println!("  4. Start working: scud next");
    } else {
        println!("  3. Start working: scud next");
    }
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    /// Helper to simulate the dependency remapping logic
    fn remap_dependencies(deps: &[String], task_ids: &[String]) -> Vec<String> {
        deps.iter()
            .filter_map(|dep| {
                if let Ok(dep_idx) = dep.parse::<usize>() {
                    if dep_idx > 0 && dep_idx <= task_ids.len() {
                        Some(task_ids[dep_idx - 1].clone())
                    } else {
                        None
                    }
                } else {
                    Some(dep.clone())
                }
            })
            .collect()
    }

    #[test]
    fn test_remap_sequential_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
        let deps = vec!["1".to_string(), "2".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        assert_eq!(result, vec!["1".to_string(), "2".to_string()]);
    }

    #[test]
    fn test_remap_uuid_deps() {
        let task_ids = vec![
            "abc123def456789012345678901234ab".to_string(),
            "def456abc789012345678901234abcde".to_string(),
            "789012345678901234abcdef12345678".to_string(),
        ];
        let deps = vec!["1".to_string(), "2".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        assert_eq!(
            result,
            vec![
                "abc123def456789012345678901234ab".to_string(),
                "def456abc789012345678901234abcde".to_string(),
            ]
        );
    }

    #[test]
    fn test_filter_zero_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string()];
        let deps = vec!["0".to_string(), "1".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        // "0" should be filtered out
        assert_eq!(result, vec!["1".to_string()]);
    }

    #[test]
    fn test_filter_out_of_range_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string()];
        let deps = vec!["1".to_string(), "99".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        // "99" should be filtered out
        assert_eq!(result, vec!["1".to_string()]);
    }

    #[test]
    fn test_preserve_cross_phase_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string()];
        let deps = vec!["1".to_string(), "auth:3".to_string()];

        let result = remap_dependencies(&deps, &task_ids);

        // "auth:3" should be preserved as-is
        assert_eq!(result, vec!["1".to_string(), "auth:3".to_string()]);
    }

    #[test]
    fn test_empty_deps() {
        let task_ids = vec!["1".to_string(), "2".to_string()];
        let deps: Vec<String> = vec![];

        let result = remap_dependencies(&deps, &task_ids);

        assert!(result.is_empty());
    }
}
