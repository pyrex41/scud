use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);

    if storage.is_initialized() {
        println!("{}", "✓ SCUD is already initialized".green());
        return Ok(());
    }

    println!("{}", "Initializing SCUD...".blue());

    storage.initialize()?;

    println!("\n{}", "✅ SCUD initialized successfully!".green().bold());
    println!("\n{}", "Next steps:".blue());
    println!("  1. Run: scud tags");
    println!("  2. Start with: /tm-pm (or use Claude Code slash command)\n");

    Ok(())
}
