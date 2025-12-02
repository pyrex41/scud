use anyhow::Result;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::models::task::TaskStatus;
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, tag: Option<&str>, limit: usize) -> Result<()> {
    let storage = Storage::new(project_root);
    let phase_tag = resolve_group_tag(&storage, tag, true)?;

    let phase = storage.load_group(&phase_tag)?;

    let ready_tasks: Vec<_> = phase
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Pending)
        .filter(|t| t.has_dependencies_met(&phase.tasks))
        .filter(|t| !t.is_locked())
        .take(limit)
        .collect();

    let output = serde_json::json!({
        "tag": phase_tag,
        "count": ready_tasks.len(),
        "tasks": ready_tasks.iter().map(|t| {
            serde_json::json!({
                "id": t.id,
                "title": t.title,
                "complexity": t.complexity,
                "priority": format!("{:?}", t.priority)
            })
        }).collect::<Vec<_>>()
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
