use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use scud_core::weave::{Decision, Event};

use super::check::{build_coordinator, core_storage};

pub fn run(project_root: Option<PathBuf>, event_json: &str) -> Result<()> {
    let storage = core_storage(project_root);
    let event: Event = serde_json::from_str(event_json)
        .map_err(|e| anyhow::anyhow!("Invalid event JSON: {}", e))?;

    let (coord, _tag, _phase) = build_coordinator(&storage)?;

    println!("{}", "Event:".blue().bold());
    println!("  Kind:   {}", event.kind);
    if let Some(ref agent) = event.agent {
        println!("  Agent:  {}", agent);
    }
    if let Some(ref target) = event.target {
        println!("  Target: {}", target);
    }
    if let Some(ref task_id) = event.task_id {
        println!("  Task:   {}", task_id);
    }
    println!();

    // Full evaluation including roles
    let decision = coord.evaluate_full(&event);

    match &decision {
        Decision::Proceed => {
            println!("{}", "PROCEED".green().bold());
            println!("No b-threads blocked this event.");

            // Show which threads were checked
            let checked: Vec<_> = coord.threads.iter().filter(|t| t.enabled).collect();
            if !checked.is_empty() {
                println!("\n{}", "Threads evaluated:".dimmed());
                for t in checked {
                    println!("  {} {} - passed", t.id.cyan(), t.name);
                }
            }
        }
        Decision::Wait { reason, thread_id } => {
            println!("{}", "WAIT".yellow().bold());
            println!("Thread: {}", thread_id.cyan());
            println!("Reason: {}", reason);
            println!("\nThe prerequisite has not been satisfied yet.");
        }
        Decision::Blocked { reason, thread_id } => {
            println!("{}", "BLOCKED".red().bold());
            println!("Thread: {}", thread_id.cyan());
            println!("Reason: {}", reason);

            // Show lock details if relevant
            if !coord.active_locks.is_empty() {
                println!("\n{}", "Active locks:".dimmed());
                for (key, lock) in &coord.active_locks {
                    println!(
                        "  {} -> {} (task: {})",
                        key,
                        lock.holder_agent,
                        lock.task_id.as_deref().unwrap_or("-"),
                    );
                }
            }
        }
    }

    Ok(())
}
