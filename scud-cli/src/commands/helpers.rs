use anyhow::Result;
use colored::Colorize;
use dialoguer::Select;

use crate::storage::Storage;

/// Check if we're running in an interactive terminal
pub fn is_interactive() -> bool {
    atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stdout)
}

/// Resolve epic tag with fallback to active epic and interactive selection
///
/// Priority:
/// 1. Explicit --tag argument
/// 2. Active epic (from workflow-state.json)
/// 3. Interactive selection (if TTY available)
/// 4. Error with helpful message
pub fn resolve_epic_tag(
    storage: &Storage,
    explicit_tag: Option<&str>,
    allow_interactive: bool,
) -> Result<String> {
    // Priority 1: Explicit --tag argument
    if let Some(tag) = explicit_tag {
        let tasks = storage.load_tasks()?;
        if !tasks.contains_key(tag) {
            anyhow::bail!("Epic '{}' not found. Run: scud tags", tag);
        }
        return Ok(tag.to_string());
    }

    // Priority 2: Active epic
    if let Some(active) = storage.get_active_epic()? {
        return Ok(active);
    }

    // Priority 3: Interactive selection
    if allow_interactive && is_interactive() {
        let tasks = storage.load_tasks()?;
        if tasks.is_empty() {
            anyhow::bail!("No epics found. Create one with: scud parse-prd <file> --tag <tag>");
        }

        let mut tags: Vec<&String> = tasks.keys().collect();
        tags.sort();

        // Show selection prompt
        println!("{}", "No active epic set.".yellow());
        let selection = Select::new()
            .with_prompt("Select an epic")
            .items(&tags)
            .default(0)
            .interact()?;

        let selected = tags[selection].clone();

        // Set as active for next time
        storage.set_active_epic(&selected)?;
        println!("{} {}", "Active epic set to:".green(), selected.green());

        return Ok(selected);
    }

    // Priority 4: Error
    anyhow::bail!("No active epic. Use --tag <epic-tag> or run: scud tags <tag>")
}
