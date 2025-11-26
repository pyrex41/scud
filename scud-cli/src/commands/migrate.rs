use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::models::task::{Task, TaskStatus};
use crate::storage::Storage;

/// Migrate task IDs to namespaced format and fix legacy patterns
pub fn run(project_root: Option<PathBuf>, dry_run: bool) -> Result<()> {
    let storage = Storage::new(project_root);

    // Check if tasks file exists
    let tasks_file = storage.tasks_file();
    if !tasks_file.exists() {
        println!("{}", "No tasks file found. Nothing to migrate.".yellow());
        return Ok(());
    }

    let mut all_tasks = storage.load_tasks()?;
    let mut changes: Vec<String> = Vec::new();
    let mut parent_fixes = 0;
    let mut subtask_links = 0;

    for (epic_tag, epic) in all_tasks.iter_mut() {
        let mut id_map: HashMap<String, String> = HashMap::new();

        // Phase 1: Collect ID mappings for tasks that need namespacing
        for task in &epic.tasks {
            if !task.id.contains(':') {
                let new_id = Task::make_id(epic_tag, &task.id);
                id_map.insert(task.id.clone(), new_id.clone());
                changes.push(format!("{}: {} -> {}", epic_tag, task.id, new_id));
            }
        }

        // Phase 2: Update IDs and references
        for task in &mut epic.tasks {
            // Update task ID if it's not namespaced
            if let Some(new_id) = id_map.get(&task.id) {
                task.id = new_id.clone();
            }

            // Update dependencies to use namespaced IDs
            task.dependencies = task
                .dependencies
                .iter()
                .map(|dep| {
                    id_map.get(dep).cloned().unwrap_or_else(|| {
                        if dep.contains(':') {
                            dep.clone()
                        } else {
                            Task::make_id(epic_tag, dep)
                        }
                    })
                })
                .collect();

            // Update parent_id if present
            if let Some(ref parent) = task.parent_id {
                task.parent_id = Some(
                    id_map
                        .get(parent)
                        .cloned()
                        .unwrap_or_else(|| Task::make_id(epic_tag, parent)),
                );
            }

            // Update subtask references
            task.subtasks = task
                .subtasks
                .iter()
                .map(|sub| {
                    id_map
                        .get(sub)
                        .cloned()
                        .unwrap_or_else(|| Task::make_id(epic_tag, sub))
                })
                .collect();

            // Fix [PARENT] prefix -> Expanded status
            if task.title.starts_with("[PARENT]") {
                task.title = task.title.trim_start_matches("[PARENT]").trim().to_string();
                task.status = TaskStatus::Expanded;
                parent_fixes += 1;
            }
        }

        // Phase 3: Infer parent-child relationships from ID patterns (e.g., 10.1 is subtask of 10)
        // First pass: collect the relationships
        let task_ids: Vec<String> = epic.tasks.iter().map(|t| t.id.clone()).collect();
        let mut parent_child_links: Vec<(String, String)> = Vec::new(); // (child_id, parent_id)

        for task in &epic.tasks {
            // If this task looks like a subtask (contains dot in local_id) and has no parent_id
            let local_id = task.local_id().to_string();
            if local_id.contains('.') && task.parent_id.is_none() {
                // Extract parent local ID (e.g., "10.1" -> "10")
                if let Some(parent_local) = local_id.rsplit_once('.').map(|(p, _)| p.to_string()) {
                    let parent_id = Task::make_id(epic_tag, &parent_local);
                    if task_ids.contains(&parent_id) {
                        parent_child_links.push((task.id.clone(), parent_id));
                    }
                }
            }
        }

        // Second pass: apply the relationships
        for (child_id, parent_id) in parent_child_links {
            // Set parent_id on child
            if let Some(child) = epic.tasks.iter_mut().find(|t| t.id == child_id) {
                child.parent_id = Some(parent_id.clone());
                subtask_links += 1;
            }
            // Add child to parent's subtasks
            if let Some(parent) = epic.tasks.iter_mut().find(|t| t.id == parent_id) {
                if !parent.subtasks.contains(&child_id) {
                    parent.subtasks.push(child_id);
                }
            }
        }
    }

    // Phase 4: Ensure parents with subtasks have Expanded status
    for (_, epic) in all_tasks.iter_mut() {
        let subtask_ids: Vec<String> = epic
            .tasks
            .iter()
            .filter(|t| t.parent_id.is_some())
            .filter_map(|t| t.parent_id.clone())
            .collect();

        for task in &mut epic.tasks {
            if subtask_ids.contains(&task.id)
                && task.status != TaskStatus::Expanded
                && (task.status == TaskStatus::Pending || task.status == TaskStatus::InProgress)
            {
                task.status = TaskStatus::Expanded;
                parent_fixes += 1;
            }
        }
    }

    if dry_run {
        println!("{}", "Dry run - no changes made".yellow());
        println!();

        if changes.is_empty() && parent_fixes == 0 && subtask_links == 0 {
            println!("{}", "No migrations needed. Data is up to date!".green());
            return Ok(());
        }

        if !changes.is_empty() {
            println!("{}", "ID changes:".blue().bold());
            for change in &changes {
                println!("  {}", change);
            }
            println!();
        }

        println!("{}", "Summary:".blue().bold());
        println!("  {} ID namespacing changes", changes.len());
        println!("  {} [PARENT] prefix fixes", parent_fixes);
        println!("  {} subtask relationships inferred", subtask_links);
    } else {
        if changes.is_empty() && parent_fixes == 0 && subtask_links == 0 {
            println!("{}", "No migrations needed. Data is up to date!".green());
            return Ok(());
        }

        storage.save_tasks(&all_tasks)?;

        println!("{}", "Migration complete!".green().bold());
        println!();
        println!("  {} task IDs namespaced", changes.len());
        println!(
            "  {} [PARENT] prefixes converted to Expanded status",
            parent_fixes
        );
        println!("  {} subtask relationships established", subtask_links);
        println!();
        println!(
            "{}",
            "Tip: Run 'scud list' to verify the migration.".dimmed()
        );
    }

    Ok(())
}
