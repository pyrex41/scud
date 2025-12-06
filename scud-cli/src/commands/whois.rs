use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, tag: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);

    // If tag provided, only show that epic. Otherwise show all epics.
    let (tasks, scope_label) = if let Some(t) = tag {
        let epic_tag = resolve_group_tag(&storage, Some(t), false)?;
        let all_tasks = storage.load_tasks()?;
        let epic = all_tasks
            .get(&epic_tag)
            .ok_or_else(|| anyhow::anyhow!("Epic '{}' not found", epic_tag))?;
        let mut filtered = HashMap::new();
        filtered.insert(epic_tag.clone(), epic.clone());
        (filtered, format!("(epic: {})", epic_tag))
    } else {
        (storage.load_tasks()?, "(all epics)".to_string())
    };

    let mut assignments: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

    // Collect all assignments across all epics
    for (epic_tag, epic) in tasks.iter() {
        for task in &epic.tasks {
            if let Some(ref assigned) = task.assigned_to {
                assignments.entry(assigned.clone()).or_default().push((
                    epic_tag.clone(),
                    task.id.clone(),
                    task.title.clone(),
                ));
            }
        }
    }

    if assignments.is_empty() {
        println!("{}", "No tasks are currently assigned".yellow());
        println!();
        println!("{}", "Assign tasks with:".blue());
        println!("  scud assign <task-id> <assignee>");
        return Ok(());
    }

    println!(
        "\n{} {}",
        "Task Assignments".blue().bold(),
        scope_label.dimmed()
    );
    println!("{}", "=".repeat(60).blue());
    println!();

    for (assignee, tasks_list) in assignments.iter() {
        println!("{} {}", "●".green(), assignee.green().bold());
        for (epic, task_id, title) in tasks_list {
            println!("  {} {} - {}", epic.cyan(), task_id.yellow(), title);
        }
        println!();
    }

    Ok(())
}
