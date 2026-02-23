//! `scud attractor import` — Import/convert pipeline formats (placeholder).

use anyhow::Result;
use std::path::Path;

/// Import a pipeline file (currently a no-op placeholder for future format conversion).
pub fn run(file: &Path, output: Option<&Path>) -> Result<()> {
    println!("Import from: {}", file.display());
    if let Some(out) = output {
        println!("Output to: {}", out.display());
    }
    println!("(Import functionality is planned for a future release)");
    Ok(())
}
