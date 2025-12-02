use anyhow::Result;
use std::path::PathBuf;

use crate::commands::helpers::resolve_group_tag;
use crate::formats::serialize_scg;
use crate::models::{Phase, TaskStatus};
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    status_filter: Option<&str>,
    tag: Option<&str>,
    json_output: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    // Resolve phase tag (explicit --tag, active phase, or interactive selection)
    let phase_tag = resolve_group_tag(&storage, tag, true)?;
    let tasks = storage.load_tasks()?;
    let phase = tasks
        .get(&phase_tag)
        .ok_or_else(|| anyhow::anyhow!("Phase '{}' not found", phase_tag))?;

    // Parse filter status once
    let filter_status = status_filter
        .map(|s| {
            TaskStatus::from_str(s).ok_or_else(|| {
                anyhow::anyhow!("Invalid status: {}. Valid: {:?}", s, TaskStatus::all())
            })
        })
        .transpose()?;

    // Create filtered phase for output
    let filtered_phase = if filter_status.is_some() {
        let filtered_tasks: Vec<_> = phase
            .tasks
            .iter()
            .filter(|t| {
                filter_status
                    .as_ref()
                    .map(|fs| t.status == *fs)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();

        let mut filtered = Phase::new(phase.name.clone());
        filtered.tasks = filtered_tasks;
        filtered
    } else {
        phase.clone()
    };

    if filtered_phase.tasks.is_empty() {
        if json_output {
            println!("[]");
        } else {
            // Output empty SCG
            println!("# SCUD Graph v1");
            println!("# Phase: {}", phase_tag);
            println!();
            println!("@nodes");
            println!("# id | title | status | complexity | priority");
            println!("# (no tasks)");
        }
        return Ok(());
    }

    if json_output {
        // JSON output
        let json = serde_json::to_string_pretty(&filtered_phase.tasks)?;
        println!("{}", json);
    } else {
        // SCG output (default)
        let scg = serialize_scg(&filtered_phase);
        print!("{}", scg);
    }

    Ok(())
}
