use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, group_id: &str, epic_tag: &str) -> Result<()> {
    let storage = Storage::new(project_root);

    // Validate epic exists
    let tasks = storage.load_tasks()?;
    if !tasks.contains_key(epic_tag) {
        anyhow::bail!("Epic '{}' not found", epic_tag);
    }

    // Load and update group
    let mut groups = storage.load_groups()?;
    let group = groups
        .get_group_mut(group_id)
        .ok_or_else(|| anyhow::anyhow!("Group '{}' not found", group_id))?;

    if group.contains_epic(epic_tag) {
        anyhow::bail!("Epic '{}' is already in group '{}'", epic_tag, group_id);
    }

    group.add_epic(epic_tag.to_string());
    storage.save_groups(&groups)?;

    println!(
        "{} Added epic {} to group {}",
        "✓".green(),
        epic_tag.cyan(),
        group_id.green()
    );

    Ok(())
}
