use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::models::EpicGroup;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    name: &str,
    epics_str: &str,
    description: Option<&str>,
) -> Result<()> {
    let storage = Storage::new(project_root);

    // Parse epic tags
    let epic_tags: Vec<String> = epics_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if epic_tags.is_empty() {
        anyhow::bail!("At least one epic tag is required");
    }

    // Validate that all epics exist
    let tasks = storage.load_tasks()?;
    for tag in &epic_tags {
        if !tasks.contains_key(tag) {
            anyhow::bail!("Epic '{}' not found", tag);
        }
    }

    // Generate group ID from name
    let group_id = name
        .to_lowercase()
        .replace(char::is_whitespace, "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");

    // Load existing groups
    let mut groups = storage.load_groups()?;

    // Check if group ID already exists
    if groups.get_group(&group_id).is_some() {
        anyhow::bail!("Group '{}' already exists", group_id);
    }

    // Create new group
    let mut group = EpicGroup::new(group_id.clone(), name.to_string(), epic_tags.clone());
    if let Some(desc) = description {
        group.description = Some(desc.to_string());
    }

    groups.add_group(group);
    storage.save_groups(&groups)?;

    println!("{}", "✅ Epic group created!".green().bold());
    println!();
    println!("{:<20} {}", "Group ID:".yellow(), group_id.cyan());
    println!("{:<20} {}", "Name:".yellow(), name);
    println!("{:<20} {}", "Epics:".yellow(), epic_tags.join(", "));
    if let Some(desc) = description {
        println!("{:<20} {}", "Description:".yellow(), desc);
    }
    println!();
    println!("{}", "Usage:".blue());
    println!("  scud group-status {}", group_id);
    println!("  scud list --group {}", group_id);
    println!("  scud stats --group {}", group_id);
    println!();

    Ok(())
}
