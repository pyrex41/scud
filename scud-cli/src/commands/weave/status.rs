use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use super::check::{build_coordinator, core_storage, resolve_tag};

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let (coord, _tag, _phase) = build_coordinator(&storage)?;

    println!("{} {}\n", "Phase:".blue().bold(), phase_tag.cyan());

    // Active locks
    if coord.active_locks.is_empty() {
        println!("{}", "No active locks".dimmed());
    } else {
        println!("{}", "Active Locks:".yellow().bold());
        for (key, lock) in &coord.active_locks {
            let expired = if lock.is_expired() {
                " (EXPIRED)".red().to_string()
            } else {
                String::new()
            };
            println!(
                "  {} -> {} (task: {}, ttl: {}s){}",
                key.cyan(),
                lock.holder_agent.green(),
                lock.task_id.as_deref().unwrap_or("-"),
                lock.ttl_secs,
                expired,
            );
        }
    }

    println!();

    // Thread summary
    let total = coord.threads.len();
    let enabled = coord.threads.iter().filter(|t| t.enabled).count();
    let disabled = total - enabled;

    println!("{}", "B-Threads:".yellow().bold());
    println!(
        "  {} total, {} enabled, {} disabled",
        total, enabled, disabled
    );

    println!();

    // Event log summary
    let log_count = coord.event_log.len();
    println!("{}", "Event Log:".yellow().bold());
    println!("  {} event(s) recorded", log_count);
    if let Some(last) = coord.event_log.last() {
        println!("  Last: {} at {}", last.event.kind, last.timestamp);
    }

    Ok(())
}
