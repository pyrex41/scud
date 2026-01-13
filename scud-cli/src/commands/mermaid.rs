use anyhow::Result;
use std::path::PathBuf;

use crate::models::task::TaskStatus;
use crate::storage::Storage;

/// Generate a Mermaid diagram of the task graph
pub fn run(project_root: Option<PathBuf>, tag: Option<&str>, all_tags: bool) -> Result<()> {
    let storage = Storage::new(project_root);
    let all_tasks = storage.load_tasks()?;

    // Determine which phase(s) to include
    let phase_tags: Vec<String> = if all_tags {
        all_tasks.keys().cloned().collect()
    } else if let Some(t) = tag {
        if !all_tasks.contains_key(t) {
            anyhow::bail!("Phase '{}' not found. Run: scud tags", t);
        }
        vec![t.to_string()]
    } else {
        // Use active phase
        let active = storage.get_active_group()?;
        match active {
            Some(t) => vec![t],
            None => anyhow::bail!("No active task group. Use --tag <phase-tag> or run: scud tags"),
        }
    };

    // Start Mermaid graph
    println!("```mermaid");
    println!("flowchart TD");

    // Add subgraph for each tag
    for tag in &phase_tags {
        if let Some(phase) = all_tasks.get(tag) {
            if phase_tags.len() > 1 {
                println!("    subgraph {}[\"Phase: {}\"]", sanitize_id(tag), tag);
            }

            // Add task nodes
            for task in &phase.tasks {
                let node_id = sanitize_id(&task.id);
                let label = escape_label(&task.title);
                let style_class = status_to_class(&task.status);

                // Format: id["title"] or id(["title"]) for different shapes
                let shape = match task.status {
                    TaskStatus::Expanded => format!("{}[[\"{}\"]]", node_id, label),
                    TaskStatus::Done => format!("{}([\"{}\"])", node_id, label),
                    TaskStatus::Blocked => format!("{}{{\"{}\"}}", node_id, label),
                    _ => format!("{}[\"{}\"]", node_id, label),
                };

                println!("    {}", shape);

                // Add class for styling
                if !style_class.is_empty() {
                    println!("    class {} {}", node_id, style_class);
                }
            }

            // Add dependency edges
            for task in &phase.tasks {
                let task_node = sanitize_id(&task.id);
                for dep in &task.dependencies {
                    let dep_node = sanitize_id(dep);
                    // dep --> task (dependency flows to dependent)
                    println!("    {} --> {}", dep_node, task_node);
                }

                // Add parent-child relationships (subtasks)
                if let Some(ref parent_id) = task.parent_id {
                    let parent_node = sanitize_id(parent_id);
                    // Dotted line for parent-child
                    println!("    {} -.-> {}", parent_node, task_node);
                }
            }

            if phase_tags.len() > 1 {
                println!("    end");
            }
        }
    }

    // Add style definitions
    println!();
    println!("    %% Status styles");
    println!("    classDef pending fill:#f9f9f9,stroke:#999,color:#333");
    println!("    classDef inprogress fill:#e3f2fd,stroke:#1976d2,color:#1976d2,stroke-width:2px");
    println!("    classDef done fill:#e8f5e9,stroke:#4caf50,color:#2e7d32");
    println!("    classDef blocked fill:#ffebee,stroke:#f44336,color:#c62828");
    println!("    classDef review fill:#fff3e0,stroke:#ff9800,color:#e65100");
    println!("    classDef expanded fill:#f3e5f5,stroke:#9c27b0,color:#6a1b9a");
    println!("    classDef deferred fill:#eceff1,stroke:#607d8b,color:#455a64");
    println!(
        "    classDef cancelled fill:#fafafa,stroke:#bdbdbd,color:#9e9e9e,stroke-dasharray: 5 5"
    );
    println!(
        "    classDef failed fill:#ffcdd2,stroke:#b71c1c,color:#b71c1c,stroke-width:3px"
    );

    println!("```");

    Ok(())
}

/// Sanitize task ID for use as Mermaid node ID
fn sanitize_id(id: &str) -> String {
    // Replace characters that are problematic in Mermaid IDs
    id.replace([':', '.', '-', ' '], "_")
}

/// Escape label text for Mermaid
fn escape_label(text: &str) -> String {
    text.replace('"', "'").replace('\n', " ")
}

/// Map task status to Mermaid class name
fn status_to_class(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::InProgress => "inprogress",
        TaskStatus::Done => "done",
        TaskStatus::Blocked => "blocked",
        TaskStatus::Review => "review",
        TaskStatus::Expanded => "expanded",
        TaskStatus::Deferred => "deferred",
        TaskStatus::Cancelled => "cancelled",
        TaskStatus::Failed => "failed",
    }
}
