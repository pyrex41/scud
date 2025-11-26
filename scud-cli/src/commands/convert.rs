//! Convert between task storage formats

use anyhow::{Context, Result};
use colored::Colorize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::formats::{parse_scg, serialize_scg, Format};
use crate::models::Phase;
use crate::storage::Storage;

pub fn run(
    project_root: Option<PathBuf>,
    from_format: &str,
    to_format: &str,
    backup: bool,
) -> Result<()> {
    let from = Format::from_extension(from_format)
        .ok_or_else(|| anyhow::anyhow!("Unknown format: {}", from_format))?;
    let to = Format::from_extension(to_format)
        .ok_or_else(|| anyhow::anyhow!("Unknown format: {}", to_format))?;

    if from == to {
        println!("{}", "Source and target formats are the same".yellow());
        return Ok(());
    }

    // SCG → JSON conversion is blocked because the CLI only reads tasks.scg
    // Converting would break all storage operations
    if from == Format::Scg && to == Format::Json {
        anyhow::bail!(
            "SCG to JSON conversion is not supported.\n\
             The SCUD CLI requires tasks.scg for storage.\n\
             Use 'scud show' or 'scud list' to view tasks."
        );
    }

    let storage = Storage::new(project_root);
    let taskmaster_dir = storage.scud_dir();
    let tasks_dir = taskmaster_dir.join("tasks");

    // Determine source file
    let source_file = tasks_dir.join(format!("tasks.{}", from.extension()));
    let target_file = tasks_dir.join(format!("tasks.{}", to.extension()));

    if !source_file.exists() {
        anyhow::bail!(
            "Source file not found: {}\nExpected format: {}",
            source_file.display(),
            from_format
        );
    }

    println!(
        "{} {} -> {}",
        "Converting".blue(),
        source_file.display(),
        target_file.display()
    );

    // Read source
    let content = fs::read_to_string(&source_file)
        .with_context(|| format!("Failed to read {}", source_file.display()))?;

    // Parse based on source format
    let phases: HashMap<String, Phase> = match from {
        Format::Json => serde_json::from_str(&content).with_context(|| "Failed to parse JSON")?,
        Format::Scg => {
            // Parse multi-phase SCG
            parse_multi_phase_scg(&content)?
        }
    };

    println!("  {} phase(s) found", phases.len());
    for (tag, phase) in &phases {
        println!("    {} {} tasks", tag.cyan(), phase.tasks.len());
    }

    // Serialize to target format
    let output = match to {
        Format::Json => {
            serde_json::to_string_pretty(&phases).with_context(|| "Failed to serialize to JSON")?
        }
        Format::Scg => {
            let mut out = String::new();
            let mut sorted_tags: Vec<_> = phases.keys().collect();
            sorted_tags.sort();

            for (i, tag) in sorted_tags.iter().enumerate() {
                if i > 0 {
                    out.push_str("\n---\n\n");
                }
                let phase = phases.get(*tag).unwrap();
                out.push_str(&serialize_scg(phase));
            }
            out
        }
    };

    // Backup if requested
    if backup && source_file.exists() {
        let backup_file = tasks_dir.join(format!("tasks.{}.backup", from.extension()));
        fs::copy(&source_file, &backup_file)
            .with_context(|| format!("Failed to create backup at {}", backup_file.display()))?;
        println!(
            "  {} Backup created: {}",
            "✓".green(),
            backup_file.display()
        );
    }

    // Write target
    fs::write(&target_file, &output)
        .with_context(|| format!("Failed to write {}", target_file.display()))?;

    // Remove source if different file
    if source_file != target_file {
        fs::remove_file(&source_file)
            .with_context(|| format!("Failed to remove old file {}", source_file.display()))?;
    }

    println!();
    println!("{}", "Conversion complete!".green().bold());
    println!();
    println!("{}", "Verify with:".blue());
    println!("  scud list");
    println!("  scud stats");

    Ok(())
}

fn parse_multi_phase_scg(content: &str) -> Result<HashMap<String, Phase>> {
    let mut phases = HashMap::new();

    // Empty content returns empty map
    if content.trim().is_empty() {
        return Ok(phases);
    }

    // Split by phase separator (---)
    let sections: Vec<&str> = content.split("\n---\n").collect();

    for section in sections {
        let section = section.trim();
        if section.is_empty() {
            continue;
        }

        let phase = parse_scg(section).with_context(|| "Failed to parse SCG section")?;

        phases.insert(phase.name.clone(), phase);
    }

    Ok(phases)
}
