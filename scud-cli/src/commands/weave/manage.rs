use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use scud_core::weave::{BThread, BThreadRule};

use super::check::{core_storage, resolve_tag};

pub fn run_init(project_root: Option<PathBuf>) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let phase = storage.load_group(&phase_tag)?;

    if !phase.weave_threads.is_empty() {
        println!(
            "@weave section already exists with {} thread(s)",
            phase.weave_threads.len()
        );
        return Ok(());
    }

    // Create weave directory for event log
    let weave_dir = storage.scud_dir().join("weave");
    std::fs::create_dir_all(&weave_dir)?;

    println!("Weave initialized for phase '{}'", phase_tag);
    println!("Add threads with: scud weave add <id> <name> <rule_type> <key=value ...>");
    Ok(())
}

pub fn run_add(
    project_root: Option<PathBuf>,
    id: &str,
    name: &str,
    rule_type: &str,
    rule_spec: &[String],
) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let mut phase = storage.load_group(&phase_tag)?;

    // Check for duplicate ID
    if phase.weave_threads.iter().any(|t| t.id == id) {
        anyhow::bail!("B-thread '{}' already exists", id);
    }

    let spec = rule_spec.join(" ");
    let rule = BThreadRule::parse(rule_type, &spec)?;

    let bthread = BThread {
        id: id.to_string(),
        name: name.to_string(),
        priority: (phase.weave_threads.len() as u32 + 1) * 10,
        enabled: true,
        rules: vec![rule],
    };

    phase.weave_threads.push(bthread);
    storage.update_group(&phase_tag, &phase)?;

    // Ensure weave directory exists
    let weave_dir = storage.scud_dir().join("weave");
    std::fs::create_dir_all(&weave_dir)?;

    println!("Added b-thread {} \"{}\" ({})", id, name, rule_type);
    Ok(())
}

pub fn run_enable(project_root: Option<PathBuf>, id: &str) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let mut phase = storage.load_group(&phase_tag)?;

    let thread = phase
        .weave_threads
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or_else(|| anyhow::anyhow!("B-thread '{}' not found", id))?;

    thread.enabled = true;
    storage.update_group(&phase_tag, &phase)?;
    println!("Enabled b-thread {}", id);
    Ok(())
}

pub fn run_disable(project_root: Option<PathBuf>, id: &str) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let mut phase = storage.load_group(&phase_tag)?;

    let thread = phase
        .weave_threads
        .iter_mut()
        .find(|t| t.id == id)
        .ok_or_else(|| anyhow::anyhow!("B-thread '{}' not found", id))?;

    thread.enabled = false;
    storage.update_group(&phase_tag, &phase)?;
    println!("Disabled b-thread {}", id);
    Ok(())
}

pub fn run_remove(project_root: Option<PathBuf>, id: &str) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let mut phase = storage.load_group(&phase_tag)?;

    let before = phase.weave_threads.len();
    phase.weave_threads.retain(|t| t.id != id);

    if phase.weave_threads.len() == before {
        anyhow::bail!("B-thread '{}' not found", id);
    }

    storage.update_group(&phase_tag, &phase)?;
    println!("Removed b-thread {}", id);
    Ok(())
}

pub fn run_list(project_root: Option<PathBuf>) -> Result<()> {
    let storage = core_storage(project_root);
    let phase_tag = resolve_tag(&storage)?;
    let phase = storage.load_group(&phase_tag)?;

    if phase.weave_threads.is_empty() {
        println!("No b-threads defined. Add one with: scud weave add <id> <name> <rule_type> <spec...>");
        return Ok(());
    }

    println!("{} {}\n", "Phase:".blue().bold(), phase_tag.cyan());
    println!(
        "{:<8} {:<24} {:>4} {:<8} {}",
        "ID".dimmed(),
        "Name".dimmed(),
        "Pri".dimmed(),
        "Status".dimmed(),
        "Rule".dimmed(),
    );
    println!("{}", "-".repeat(70).dimmed());

    for thread in &phase.weave_threads {
        let status = if thread.enabled {
            "ON".green().to_string()
        } else {
            "OFF".red().to_string()
        };

        let rule_desc = thread
            .rules
            .first()
            .map(format_rule_brief)
            .unwrap_or_default();

        println!(
            "{:<8} {:<24} {:>4} {:<8} {}",
            thread.id.cyan(),
            thread.name,
            thread.priority,
            status,
            rule_desc.dimmed(),
        );
    }

    println!(
        "\n{} {} thread(s)",
        "Total:".dimmed(),
        phase.weave_threads.len()
    );
    Ok(())
}

fn format_rule_brief(rule: &BThreadRule) -> String {
    match rule {
        BThreadRule::Mutex { key, .. } => format!("Mutex({})", key),
        BThreadRule::Require { .. } => "Require".to_string(),
        BThreadRule::BlockUntil { .. } => "BlockUntil".to_string(),
        BThreadRule::BlockAlways { .. } => "BlockAlways".to_string(),
        BThreadRule::RateLimit {
            max, window_secs, ..
        } => {
            format!("RateLimit({}/{}s)", max, window_secs)
        }
        BThreadRule::Timeout {
            max_duration_secs, ..
        } => format!("Timeout({}s)", max_duration_secs),
        BThreadRule::Partition {
            strategy,
            agent_count,
            ..
        } => format!("Partition({:?}/{})", strategy, agent_count),
    }
}
