use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::path::PathBuf;

use scud_core::weave::EventLog;

use super::check::{build_coordinator, core_storage, resolve_tag};

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let (coord, _tag, _phase) = build_coordinator(&storage)?;

    println!("{}", "Weave Summary".blue().bold());
    println!("{}", "=".repeat(50).dimmed());
    println!("Phase: {}\n", phase_tag.cyan());

    // B-thread overview
    let total = coord.threads.len();
    let enabled = coord.threads.iter().filter(|t| t.enabled).count();
    println!(
        "{} {} total, {} enabled",
        "Threads:".yellow(),
        total,
        enabled
    );
    for thread in &coord.threads {
        let status = if thread.enabled { "ON" } else { "off" };
        println!("  {} {} [{}]", thread.id.cyan(), thread.name, status);
    }

    println!();

    // Active locks
    println!("{} {}", "Locks:".yellow(), coord.active_locks.len());
    for (key, lock) in &coord.active_locks {
        let expired = if lock.is_expired() { " EXPIRED" } else { "" };
        println!(
            "  {} -> {}{}",
            key.cyan(),
            lock.holder_agent.green(),
            expired.red(),
        );
    }

    println!();

    // Active agents (from recent events)
    let mut agents: HashMap<&str, usize> = HashMap::new();
    for te in &coord.event_log {
        if let Some(ref agent) = te.event.agent {
            *agents.entry(agent.as_str()).or_default() += 1;
        }
    }
    println!("{} {} unique", "Agents:".yellow(), agents.len());
    let mut agent_list: Vec<_> = agents.iter().collect();
    agent_list.sort_by(|a, b| b.1.cmp(a.1));
    for (agent, count) in agent_list {
        println!("  {} ({} events)", agent.green(), count);
    }

    println!();

    // Recent events
    let weave_dir = storage.scud_dir().join("weave");
    let event_log = EventLog::load(&weave_dir.join("events.jsonl"))?;
    let recent_count = 5;
    let start = event_log.events.len().saturating_sub(recent_count);
    let recent = &event_log.events[start..];

    println!("{} (last {})", "Recent Events:".yellow(), recent.len());
    for te in recent {
        let agent = te.event.agent.as_deref().unwrap_or("-");
        println!(
            "  {} {} [{}]",
            te.timestamp.dimmed(),
            te.event.kind.to_string().cyan(),
            agent,
        );
    }

    // Roles summary
    if !coord.roles.is_empty() {
        println!();
        println!("{} {}", "Roles:".yellow(), coord.roles.len());
        for role in &coord.roles {
            println!("  {} ({})", role.id.cyan(), role.name);
        }
    }

    // Partitions summary
    if !coord.partitions.is_empty() {
        println!();
        println!("{} {}", "Partitions:".yellow(), coord.partitions.len());
        for part in &coord.partitions {
            println!(
                "  {} {:?} ({} agents)",
                part.id.cyan(),
                part.strategy,
                part.agent_count,
            );
        }
    }

    Ok(())
}
