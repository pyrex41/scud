use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;

pub fn run(project_root: Option<PathBuf>, action: &str) -> Result<()> {
    match action {
        "install" => install_hooks(project_root)?,
        "uninstall" => uninstall_hooks(project_root)?,
        "status" => show_status(project_root)?,
        _ => {
            println!("Usage: scud hooks <install|uninstall|status>");
            println!();
            println!("Commands:");
            println!("  install    Install Claude Code hooks for automatic task completion");
            println!("  uninstall  Remove Claude Code hooks");
            println!("  status     Show current hook installation status");
        }
    }
    Ok(())
}

fn get_settings_path(project_root: Option<PathBuf>) -> PathBuf {
    let root = project_root.unwrap_or_else(|| std::env::current_dir().unwrap());
    root.join(".claude/settings.local.json")
}

fn install_hooks(project_root: Option<PathBuf>) -> Result<()> {
    let settings_path = get_settings_path(project_root);

    // Ensure .claude directory exists
    if let Some(parent) = settings_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Load existing settings or create new
    let mut settings: Value = if settings_path.exists() {
        let content = fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content).unwrap_or(json!({}))
    } else {
        json!({})
    };

    // Add hooks configuration
    let hooks = json!({
        "Stop": [{
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "scud _hook-complete"
            }]
        }]
    });

    settings["hooks"] = hooks;

    // Write back
    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, content)?;

    println!("✓ Claude Code hooks installed");
    println!();
    println!("Active hooks:");
    println!("  • Stop → scud _hook-complete (enforces task completion)");
    println!();
    println!("Hooks are stored in: .claude/settings.local.json");
    Ok(())
}

fn uninstall_hooks(project_root: Option<PathBuf>) -> Result<()> {
    let settings_path = get_settings_path(project_root);

    if !settings_path.exists() {
        println!("No hooks installed (settings file not found)");
        return Ok(());
    }

    let content = fs::read_to_string(&settings_path)?;
    let mut settings: Value = serde_json::from_str(&content)?;

    if let Some(obj) = settings.as_object_mut() {
        obj.remove("hooks");
    }

    let content = serde_json::to_string_pretty(&settings)?;
    fs::write(&settings_path, content)?;

    println!("✓ Claude Code hooks uninstalled");
    Ok(())
}

fn show_status(project_root: Option<PathBuf>) -> Result<()> {
    let settings_path = get_settings_path(project_root);

    if !settings_path.exists() {
        println!("Hooks: NOT INSTALLED");
        println!();
        println!("Run: scud hooks install");
        return Ok(());
    }

    let content = fs::read_to_string(&settings_path)?;
    let settings: Value = serde_json::from_str(&content)?;

    if settings.get("hooks").is_some() {
        println!("Hooks: INSTALLED");
        println!();
        println!("Active hooks:");
        println!("  • Stop → scud _hook-complete");
        println!();
        println!("Settings file: .claude/settings.local.json");
    } else {
        println!("Hooks: NOT INSTALLED");
        println!();
        println!("Run: scud hooks install");
    }

    Ok(())
}
