use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use scud_core::weave::EventLog;

use super::check::core_storage;

pub fn run(project_root: Option<PathBuf>, tail: usize) -> Result<()> {
    let storage = core_storage(project_root);
    let weave_dir = storage.scud_dir().join("weave");
    let log_path = weave_dir.join("events.jsonl");

    let event_log = EventLog::load(&log_path)?;

    if event_log.events.is_empty() {
        println!("{}", "No events recorded yet.".dimmed());
        return Ok(());
    }

    let start = event_log.events.len().saturating_sub(tail);
    let events = &event_log.events[start..];

    println!(
        "{} (showing last {} of {})\n",
        "Event Log".blue().bold(),
        events.len(),
        event_log.events.len()
    );

    for te in events {
        let agent = te.event.agent.as_deref().unwrap_or("-");
        let target = te.event.target.as_deref().unwrap_or("");
        let target_display = if target.is_empty() {
            String::new()
        } else {
            format!(" -> {}", target)
        };

        println!(
            "  {} {} [{}]{}",
            te.timestamp.dimmed(),
            te.event.kind.to_string().cyan(),
            agent.green(),
            target_display,
        );
    }

    Ok(())
}
