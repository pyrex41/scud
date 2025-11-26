use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use crate::storage::Storage;

/// SCUD agent definitions
/// Each agent has a filename, aliases for CLI, and description
/// Agents are stored in .claude/commands/scud/<filename>.md
const SCUD_AGENTS: &[(&str, &[&str], &str)] = &[
    ("pm", &["pm", "scud-pm"], "Product Manager - PRD creation and requirements"),
    ("sm", &["sm", "scud-sm"], "Scrum Master - Task breakdown and planning"),
    ("architect", &["architect", "scud-architect"], "Architect - Technical design"),
    ("dev", &["dev", "scud-dev"], "Developer - Task implementation"),
    ("retrospective", &["retrospective", "scud-retrospective"], "Retrospective - Post-phase analysis"),
    ("status", &["status"], "Status - Workflow status reporting"),
];

pub fn show(project_root: Option<PathBuf>) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        println!("{}", "✗ SCUD is not initialized".red());
        println!("Run: scud init");
        return Ok(());
    }

    let config = storage.load_config()?;

    println!("{}", "Current Configuration:".blue().bold());
    println!();
    println!("  {}: {}", "Provider".yellow(), config.llm.provider);
    println!("  {}: {}", "Model".yellow(), config.llm.model);
    println!("  {}: {}", "Max Tokens".yellow(), config.llm.max_tokens);
    println!();
    println!("{}", "Environment Variable:".blue().bold());
    println!("  {}: {}", "Required".yellow(), config.api_key_env_var());

    // Check if API key is set
    match std::env::var(config.api_key_env_var()) {
        Ok(key) => {
            let masked = format!(
                "{}...{}",
                &key[..10.min(key.len())],
                &key[key.len().saturating_sub(4)..]
            );
            println!(
                "  {}: {} {}",
                "Status".yellow(),
                "Set".green(),
                masked.dimmed()
            );
        }
        Err(_) => {
            println!(
                "  {}: {} (run: export {}=your-key)",
                "Status".yellow(),
                "Not Set".red(),
                config.api_key_env_var()
            );
        }
    }

    println!();
    println!("{}", "Config File:".blue().bold());
    println!("  {}", storage.config_file().display().to_string().dimmed());

    Ok(())
}

pub fn set_provider(
    project_root: Option<PathBuf>,
    provider: &str,
    model: Option<String>,
) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD is not initialized. Run: scud init");
    }

    // Validate provider
    let provider = provider.to_lowercase();
    if !matches!(
        provider.as_str(),
        "xai" | "anthropic" | "openai" | "openrouter" | "claude-cli"
    ) {
        anyhow::bail!(
            "Invalid provider: {}. Valid options: xai, anthropic, openai, openrouter, claude-cli",
            provider
        );
    }

    let mut config = storage.load_config()?;
    config.llm.provider = provider.clone();

    // Set model - use provided or default for provider
    config.llm.model =
        model.unwrap_or_else(|| Config::default_model_for_provider(&provider).to_string());

    // Save config
    config.save(&storage.config_file())?;

    println!("{}", "✅ Configuration updated!".green().bold());
    println!();
    println!("  {}: {}", "Provider".yellow(), config.llm.provider);
    println!("  {}: {}", "Model".yellow(), config.llm.model);
    println!();

    if config.requires_api_key() {
        println!("{}", "Remember to set your API key:".blue());
        println!(
            "  export {}=your-api-key",
            config.api_key_env_var().yellow()
        );
    } else {
        println!("{}", "Using Claude CLI (no API key required)".green());
        println!(
            "{}",
            "Make sure 'claude' command is available in your PATH".blue()
        );
    }

    Ok(())
}

/// Normalize agent name - accepts aliases like scud-pm, pm, architect, etc.
fn normalize_agent_name(name: &str) -> Option<&'static str> {
    let name_lower = name.to_lowercase();
    for (filename, aliases, _) in SCUD_AGENTS {
        for alias in *aliases {
            if name_lower == *alias {
                return Some(filename);
            }
        }
    }
    None
}

/// Get the scud commands directory path (.claude/commands/scud/)
fn get_scud_commands_dir(project_root: Option<PathBuf>) -> PathBuf {
    let base = project_root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join(".claude").join("commands").join("scud")
}

/// Get the package's agent source directory (.claude/commands/scud/)
fn get_package_agents_dir() -> Option<PathBuf> {
    // Try to find the scud-task npm package directory
    // The package is identified by having bin/install.js (the npm package's install script)
    // This handles both development (repo root) and installed (node_modules) scenarios
    let current_exe = std::env::current_exe().ok()?;

    // Search up from the executable location
    let mut search_dir = current_exe.parent()?;

    // Search up the directory tree for the npm package root
    for _ in 0..10 {
        let install_script = search_dir.join("bin").join("install.js");
        let scud_dir = search_dir.join(".claude").join("commands").join("scud");

        // Found the scud-task npm package (has bin/install.js and scud agents)
        if install_script.exists() && scud_dir.exists() && scud_dir.join("pm.md").exists() {
            return Some(scud_dir);
        }
        search_dir = search_dir.parent()?;
    }

    None
}

/// List installed SCUD agents
pub fn agents_list(project_root: Option<PathBuf>) -> Result<()> {
    let scud_dir = get_scud_commands_dir(project_root);

    println!("{}", "SCUD Workflow Agents".blue().bold());
    println!("{}", "Location: .claude/commands/scud/".dimmed());
    println!();

    let mut installed_count = 0;
    let mut not_installed_count = 0;

    for (filename, aliases, description) in SCUD_AGENTS {
        let agent_file = scud_dir.join(format!("{}.md", filename));
        let installed = agent_file.exists();
        let alias_str = aliases.join(", ");

        if installed {
            installed_count += 1;
            println!(
                "  {} {} ({}) - {}",
                "✓".green(),
                filename.green(),
                alias_str.dimmed(),
                description
            );
        } else {
            not_installed_count += 1;
            println!(
                "  {} {} ({}) - {}",
                "✗".red(),
                filename.dimmed(),
                alias_str.dimmed(),
                description
            );
        }
    }

    println!();
    println!(
        "{} installed, {} not installed",
        installed_count.to_string().green(),
        not_installed_count.to_string().yellow()
    );

    println!();
    println!("{}", "Usage:".blue().bold());
    println!("  scud config agents add <name>     Add an agent (e.g., pm, architect)");
    println!("  scud config agents add --all      Add all agents");
    println!("  scud config agents remove <name>  Remove an agent");
    println!("  scud config agents remove --all   Remove all agents");

    Ok(())
}

/// Add SCUD agent(s)
pub fn agents_add(project_root: Option<PathBuf>, name: Option<String>, all: bool) -> Result<()> {
    if !all && name.is_none() {
        anyhow::bail!("Please specify an agent name or use --all to add all agents");
    }

    let package_dir = get_package_agents_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find SCUD package agent files. Make sure scud-task is installed."))?;

    let scud_dir = get_scud_commands_dir(project_root);

    // Ensure scud commands directory exists
    fs::create_dir_all(&scud_dir)?;

    let agents_to_add: Vec<&str> = if all {
        SCUD_AGENTS.iter().map(|(filename, _, _)| *filename).collect()
    } else {
        let agent_name = name.as_ref().unwrap();
        let normalized = normalize_agent_name(agent_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Unknown agent: '{}'. Valid agents: pm, sm, architect, dev, retrospective, status",
                agent_name
            ))?;
        vec![normalized]
    };

    let mut added = 0;
    let mut already_exists = 0;

    for agent_name in &agents_to_add {
        let source = package_dir.join(format!("{}.md", agent_name));
        let dest = scud_dir.join(format!("{}.md", agent_name));

        if dest.exists() {
            already_exists += 1;
            println!("  {} {} (already installed)", "·".yellow(), agent_name);
            continue;
        }

        if !source.exists() {
            println!("  {} {} (source not found)", "✗".red(), agent_name);
            continue;
        }

        fs::copy(&source, &dest)?;
        added += 1;
        println!("  {} {}", "✓".green(), agent_name.green());
    }

    println!();
    if added > 0 {
        println!("{}", format!("✅ Added {} agent(s)", added).green().bold());
    }
    if already_exists > 0 {
        println!("{}", format!("{} agent(s) already installed", already_exists).yellow());
    }

    Ok(())
}

/// Remove SCUD agent(s)
pub fn agents_remove(project_root: Option<PathBuf>, name: Option<String>, all: bool) -> Result<()> {
    if !all && name.is_none() {
        anyhow::bail!("Please specify an agent name or use --all to remove all agents");
    }

    let scud_dir = get_scud_commands_dir(project_root);

    let agents_to_remove: Vec<&str> = if all {
        SCUD_AGENTS.iter().map(|(filename, _, _)| *filename).collect()
    } else {
        let agent_name = name.as_ref().unwrap();
        let normalized = normalize_agent_name(agent_name)
            .ok_or_else(|| anyhow::anyhow!(
                "Unknown agent: '{}'. Valid agents: pm, sm, architect, dev, retrospective, status",
                agent_name
            ))?;
        vec![normalized]
    };

    let mut removed = 0;
    let mut not_found = 0;

    for agent_name in &agents_to_remove {
        let agent_file = scud_dir.join(format!("{}.md", agent_name));

        if !agent_file.exists() {
            not_found += 1;
            println!("  {} {} (not installed)", "·".yellow(), agent_name);
            continue;
        }

        fs::remove_file(&agent_file)?;
        removed += 1;
        println!("  {} {}", "✓".green(), agent_name);
    }

    println!();
    if removed > 0 {
        println!("{}", format!("✅ Removed {} agent(s)", removed).green().bold());
    }
    if not_found > 0 {
        println!("{}", format!("{} agent(s) were not installed", not_found).yellow());
    }

    Ok(())
}
