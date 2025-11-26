use anyhow::Result;
use colored::Colorize;
use dialoguer::Select;
use std::path::PathBuf;

use crate::commands::helpers::is_interactive;
use crate::storage::Storage;

/// List epic tags or set active tag
///
/// Usage:
///   scud tags         - List all tags, prompt to select if interactive
///   scud tags <tag>   - Set active tag
pub fn run(project_root: Option<PathBuf>, set_tag: Option<&str>) -> Result<()> {
    let storage = Storage::new(project_root);
    let tasks = storage.load_tasks()?;

    if tasks.is_empty() {
        println!("{}", "No epics found.".yellow());
        println!("Create one with: scud parse-prd <file> --tag <tag>");
        return Ok(());
    }

    // If tag provided, set it as active (absorbs use-tag functionality)
    if let Some(tag) = set_tag {
        if !tasks.contains_key(tag) {
            anyhow::bail!("Epic '{}' not found", tag);
        }
        storage.set_active_epic(tag)?;
        println!("{} {}", "Active epic:".green(), tag.green().bold());

        if let Some(epic) = tasks.get(tag) {
            let stats = epic.get_stats();
            println!(
                "  {} tasks ({} pending, {} in-progress, {} done)",
                stats.total, stats.pending, stats.in_progress, stats.done
            );
        }
        return Ok(());
    }

    // Display all tags
    let active_epic = storage.get_active_epic()?;
    println!("{}", "Epic Tags:".blue().bold());
    println!();

    let mut tag_list: Vec<&String> = tasks.keys().collect();
    tag_list.sort();

    for (idx, tag) in tag_list.iter().enumerate() {
        let epic = tasks.get(*tag).unwrap();
        let stats = epic.get_stats();
        let is_active = active_epic.as_ref() == Some(*tag);

        let indicator = if is_active {
            "●".green()
        } else {
            "○".white()
        };
        let tag_display = if is_active {
            tag.green().bold()
        } else {
            tag.normal()
        };

        println!(
            "  {} [{}] {} ({} tasks, {} pending, {} done)",
            indicator,
            idx + 1,
            tag_display,
            stats.total,
            stats.pending,
            stats.done
        );
    }

    println!();

    // Interactive selection if no active epic or user is in interactive mode
    if is_interactive() {
        let default_idx = active_epic
            .as_ref()
            .and_then(|a| tag_list.iter().position(|t| *t == a))
            .unwrap_or(0);

        let selection = Select::new()
            .with_prompt("Select epic to activate (Ctrl+C to cancel)")
            .items(&tag_list)
            .default(default_idx)
            .interact_opt()?;

        if let Some(idx) = selection {
            let selected = tag_list[idx];
            storage.set_active_epic(selected)?;
            println!("\n{} {}", "Active epic:".green(), selected.green().bold());
        }
    } else if active_epic.is_none() {
        println!("{}", "Set active epic: scud tags <tag>".yellow());
    }

    Ok(())
}
