use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);
    let groups = storage.load_groups()?;

    if groups.groups.is_empty() {
        println!("{}", "No epic groups found".yellow());
        println!("Create a group with: scud create-group <name> --epics <tag1>,<tag2>");
        return Ok(());
    }

    println!("{}", "Epic Groups:".blue().bold());
    println!();

    for group in &groups.groups {
        let status_icon = match group.status {
            crate::models::GroupStatus::Active => "●".green(),
            crate::models::GroupStatus::Completed => "✓".blue(),
            crate::models::GroupStatus::Archived => "□".white(),
        };

        println!("{} {} {}", status_icon, group.name.bold(), format!("({})", group.id).white());
        println!("  Epics: {}", group.epic_tags.join(", ").cyan());
        if let Some(ref desc) = group.description {
            println!("  {}", desc.white());
        }
        println!();
    }

    Ok(())
}
