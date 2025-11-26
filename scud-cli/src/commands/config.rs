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
    (
        "pm",
        &["pm", "scud-pm"],
        "Product Manager - PRD creation and requirements",
    ),
    (
        "sm",
        &["sm", "scud-sm"],
        "Scrum Master - Task breakdown and planning",
    ),
    (
        "architect",
        &["architect", "scud-architect"],
        "Architect - Technical design",
    ),
    (
        "dev",
        &["dev", "scud-dev"],
        "Developer - Task implementation",
    ),
    (
        "retrospective",
        &["retrospective", "scud-retrospective"],
        "Retrospective - Post-phase analysis",
    ),
    ("status", &["status"], "Status - Workflow status reporting"),
];

/// SCUD skill definitions
/// Each skill is a directory containing SKILL.md and supporting files
/// Skills are stored in .claude/skills/<skill-name>/
const SCUD_SKILLS: &[(&str, &[&str], &str)] = &[(
    "scud-tasks",
    &["scud-tasks", "tasks"],
    "Task management - view, update, claim, and track tasks",
)];

/// SCUD task command definitions (Claude Code slash commands)
/// These wrap the CLI for common task operations
/// Commands are stored in .claude/commands/scud/
#[allow(dead_code)]
const SCUD_TASK_COMMANDS: &[&str] = &[
    "task-list",
    "task-next",
    "task-show",
    "task-status",
    "task-claim",
    "task-waves",
    "task-stats",
    "task-whois",
    "task-tags",
    "task-doctor",
];

/// OpenCode command definitions
/// These are the same commands but for OpenCode
/// Commands are stored in .opencode/command/
const OPENCODE_COMMANDS: &[&str] = &[
    "task-list",
    "task-next",
    "task-show",
    "task-status",
    "task-claim",
    "task-release",
    "task-waves",
    "task-stats",
    "task-whois",
    "task-tags",
    "task-doctor",
];

/// OpenCode hook definitions
const OPENCODE_HOOKS: &[&str] = &["session-start"];

/// OpenCode tool definitions
const OPENCODE_TOOLS: &[&str] = &["find_skills", "use_skill"];

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

/// Normalize skill name - accepts aliases like scud-tasks, tasks, etc.
fn normalize_skill_name(name: &str) -> Option<&'static str> {
    let name_lower = name.to_lowercase();
    for (dirname, aliases, _) in SCUD_SKILLS {
        for alias in *aliases {
            if name_lower == *alias {
                return Some(dirname);
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

/// Get the skills directory path (.claude/skills/)
fn get_skills_dir(project_root: Option<PathBuf>) -> PathBuf {
    let base = project_root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join(".claude").join("skills")
}

/// Get the OpenCode command directory path (.opencode/command/)
fn get_opencode_command_dir(project_root: Option<PathBuf>) -> PathBuf {
    let base = project_root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join(".opencode").join("command")
}

/// Get the OpenCode hook directory path (.opencode/hook/)
fn get_opencode_hook_dir(project_root: Option<PathBuf>) -> PathBuf {
    let base = project_root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join(".opencode").join("hook")
}

/// Get the OpenCode tool directory path (.opencode/tool/)
fn get_opencode_tool_dir(project_root: Option<PathBuf>) -> PathBuf {
    let base = project_root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join(".opencode").join("tool")
}

/// Get the OpenCode skills directory path (.opencode/skills/)
fn get_opencode_skills_dir(project_root: Option<PathBuf>) -> PathBuf {
    let base = project_root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join(".opencode").join("skills")
}

/// Get the package root directory (contains .claude/commands/scud/ and .claude/skills/)
fn get_package_root_dir() -> Option<PathBuf> {
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
            return Some(search_dir.to_path_buf());
        }
        search_dir = search_dir.parent()?;
    }

    None
}

/// Get the package's agent source directory (.claude/commands/scud/)
fn get_package_agents_dir() -> Option<PathBuf> {
    get_package_root_dir().map(|root| root.join(".claude").join("commands").join("scud"))
}

/// Get the package's skills source directory (.claude/skills/)
fn get_package_skills_dir() -> Option<PathBuf> {
    get_package_root_dir().map(|root| root.join(".claude").join("skills"))
}

/// Get the package's OpenCode command source directory (.opencode/command/)
fn get_package_opencode_command_dir() -> Option<PathBuf> {
    get_package_root_dir().map(|root| root.join(".opencode").join("command"))
}

/// Get the package's OpenCode hook source directory (.opencode/hook/)
fn get_package_opencode_hook_dir() -> Option<PathBuf> {
    get_package_root_dir().map(|root| root.join(".opencode").join("hook"))
}

/// Get the package's OpenCode tool source directory (.opencode/tool/)
fn get_package_opencode_tool_dir() -> Option<PathBuf> {
    get_package_root_dir().map(|root| root.join(".opencode").join("tool"))
}

/// List installed SCUD agents
pub fn agents_list(project_root: Option<PathBuf>) -> Result<()> {
    let scud_dir = get_scud_commands_dir(project_root.clone());
    let skills_dir = get_skills_dir(project_root.clone());

    // Agents section
    println!("{}", "SCUD Workflow Agents".blue().bold());
    println!("{}", "Location: .claude/commands/scud/".dimmed());
    println!();

    let mut agents_installed = 0;
    let mut agents_not_installed = 0;

    for (filename, aliases, description) in SCUD_AGENTS {
        let agent_file = scud_dir.join(format!("{}.md", filename));
        let installed = agent_file.exists();
        let alias_str = aliases.join(", ");

        if installed {
            agents_installed += 1;
            println!(
                "  {} {} ({}) - {}",
                "✓".green(),
                filename.green(),
                alias_str.dimmed(),
                description
            );
        } else {
            agents_not_installed += 1;
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
        agents_installed.to_string().green(),
        agents_not_installed.to_string().yellow()
    );

    // Skills section
    println!();
    println!("{}", "SCUD Skills".blue().bold());
    println!("{}", "Location: .claude/skills/".dimmed());
    println!();

    let mut skills_installed = 0;
    let mut skills_not_installed = 0;

    for (dirname, aliases, description) in SCUD_SKILLS {
        let skill_dir = skills_dir.join(dirname);
        let skill_file = skill_dir.join("SKILL.md");
        let installed = skill_file.exists();
        let alias_str = aliases.join(", ");

        if installed {
            skills_installed += 1;
            println!(
                "  {} {} ({}) - {}",
                "✓".green(),
                dirname.green(),
                alias_str.dimmed(),
                description
            );
        } else {
            skills_not_installed += 1;
            println!(
                "  {} {} ({}) - {}",
                "✗".red(),
                dirname.dimmed(),
                alias_str.dimmed(),
                description
            );
        }
    }

    println!();
    println!(
        "{} installed, {} not installed",
        skills_installed.to_string().green(),
        skills_not_installed.to_string().yellow()
    );

    // OpenCode section
    println!();
    println!("{}", "OpenCode Integration".blue().bold());
    println!("{}", "Location: .opencode/".dimmed());
    println!();

    let opencode_cmd_dir = get_opencode_command_dir(project_root.clone());
    let opencode_hook_dir = get_opencode_hook_dir(project_root.clone());
    let opencode_tool_dir = get_opencode_tool_dir(project_root);

    let mut opencode_installed = 0;
    #[allow(unused_assignments)]
    let mut opencode_not_installed = 0;

    // Check commands
    for cmd in OPENCODE_COMMANDS {
        let cmd_file = opencode_cmd_dir.join(format!("{}.md", cmd));
        if cmd_file.exists() {
            opencode_installed += 1;
        } else {
            opencode_not_installed += 1;
        }
    }

    // Check hooks
    for hook in OPENCODE_HOOKS {
        let hook_file = opencode_hook_dir.join(format!("{}.md", hook));
        if hook_file.exists() {
            opencode_installed += 1;
        } else {
            opencode_not_installed += 1;
        }
    }

    // Check tools
    for tool in OPENCODE_TOOLS {
        let tool_file = opencode_tool_dir.join(format!("{}.json", tool));
        if tool_file.exists() {
            opencode_installed += 1;
        } else {
            opencode_not_installed += 1;
        }
    }

    if opencode_installed > 0 {
        println!(
            "  {} {} commands, {} hooks, {} tools installed",
            "✓".green(),
            OPENCODE_COMMANDS
                .iter()
                .filter(|c| opencode_cmd_dir.join(format!("{}.md", c)).exists())
                .count(),
            OPENCODE_HOOKS
                .iter()
                .filter(|h| opencode_hook_dir.join(format!("{}.md", h)).exists())
                .count(),
            OPENCODE_TOOLS
                .iter()
                .filter(|t| opencode_tool_dir.join(format!("{}.json", t)).exists())
                .count(),
        );
    } else {
        println!("  {} Not installed", "✗".red());
    }

    println!();
    println!("{}", "Usage:".blue().bold());
    println!("  scud config agents add <name>     Add an agent or skill");
    println!("  scud config agents add --all      Add all agents, skills, and OpenCode support");
    println!("  scud config agents remove <name>  Remove an agent or skill");
    println!("  scud config agents remove --all   Remove all agents, skills, and OpenCode support");

    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Add SCUD agent(s), skill(s), and OpenCode integration
pub fn agents_add(project_root: Option<PathBuf>, name: Option<String>, all: bool) -> Result<()> {
    if !all && name.is_none() {
        anyhow::bail!("Please specify an agent/skill name or use --all to add all");
    }

    let package_agents_dir = get_package_agents_dir().ok_or_else(|| {
        anyhow::anyhow!(
            "Could not find SCUD package agent files. Make sure scud-task is installed."
        )
    })?;

    let package_skills_dir = get_package_skills_dir();
    let package_opencode_cmd_dir = get_package_opencode_command_dir();
    let package_opencode_hook_dir = get_package_opencode_hook_dir();
    let package_opencode_tool_dir = get_package_opencode_tool_dir();

    let scud_dir = get_scud_commands_dir(project_root.clone());
    let skills_dir = get_skills_dir(project_root.clone());
    let opencode_cmd_dir = get_opencode_command_dir(project_root.clone());
    let opencode_hook_dir = get_opencode_hook_dir(project_root.clone());
    let opencode_tool_dir = get_opencode_tool_dir(project_root.clone());
    let opencode_skills_dir = get_opencode_skills_dir(project_root);

    // Ensure directories exist
    fs::create_dir_all(&scud_dir)?;
    fs::create_dir_all(&skills_dir)?;

    let mut agents_added = 0;
    let mut agents_already_exist = 0;
    let mut skills_added = 0;
    let mut skills_already_exist = 0;
    let mut opencode_added = 0;
    let mut opencode_already_exist = 0;

    // Determine what to add
    let (agents_to_add, skills_to_add): (Vec<&str>, Vec<&str>) = if all {
        (
            SCUD_AGENTS
                .iter()
                .map(|(filename, _, _)| *filename)
                .collect(),
            SCUD_SKILLS.iter().map(|(dirname, _, _)| *dirname).collect(),
        )
    } else {
        let name_ref = name.as_ref().unwrap();
        // Try agent first, then skill
        if let Some(agent) = normalize_agent_name(name_ref) {
            (vec![agent], vec![])
        } else if let Some(skill) = normalize_skill_name(name_ref) {
            (vec![], vec![skill])
        } else {
            anyhow::bail!(
                "Unknown agent/skill: '{}'. Valid agents: pm, sm, architect, dev, retrospective, status. Valid skills: scud-tasks",
                name_ref
            );
        }
    };

    // Add agents
    if !agents_to_add.is_empty() {
        println!("{}", "Agents:".blue().bold());
        for agent_name in &agents_to_add {
            let source = package_agents_dir.join(format!("{}.md", agent_name));
            let dest = scud_dir.join(format!("{}.md", agent_name));

            if dest.exists() {
                agents_already_exist += 1;
                println!("  {} {} (already installed)", "·".yellow(), agent_name);
                continue;
            }

            if !source.exists() {
                println!("  {} {} (source not found)", "✗".red(), agent_name);
                continue;
            }

            fs::copy(&source, &dest)?;
            agents_added += 1;
            println!("  {} {}", "✓".green(), agent_name.green());
        }
    }

    // Add skills
    if !skills_to_add.is_empty() {
        if let Some(ref pkg_skills) = package_skills_dir {
            println!("{}", "Skills:".blue().bold());
            for skill_name in &skills_to_add {
                let source = pkg_skills.join(skill_name);
                let dest = skills_dir.join(skill_name);

                if dest.join("SKILL.md").exists() {
                    skills_already_exist += 1;
                    println!("  {} {} (already installed)", "·".yellow(), skill_name);
                    continue;
                }

                if !source.exists() || !source.join("SKILL.md").exists() {
                    println!("  {} {} (source not found)", "✗".red(), skill_name);
                    continue;
                }

                copy_dir_recursive(&source, &dest)?;
                skills_added += 1;
                println!("  {} {}", "✓".green(), skill_name.green());

                // Also copy skill to OpenCode skills directory
                let opencode_dest = opencode_skills_dir.join(skill_name);
                if !opencode_dest.join("SKILL.md").exists() {
                    fs::create_dir_all(&opencode_skills_dir)?;
                    copy_dir_recursive(&source, &opencode_dest)?;
                }
            }
        } else if !skills_to_add.is_empty() {
            println!(
                "{}",
                "Skills directory not found in package".yellow().dimmed()
            );
        }
    }

    // Add OpenCode integration (only when --all)
    if all {
        println!("{}", "OpenCode:".blue().bold());

        // Ensure OpenCode directories exist
        fs::create_dir_all(&opencode_cmd_dir)?;
        fs::create_dir_all(&opencode_hook_dir)?;
        fs::create_dir_all(&opencode_tool_dir)?;

        // Add commands
        if let Some(ref pkg_cmd_dir) = package_opencode_cmd_dir {
            for cmd in OPENCODE_COMMANDS {
                let source = pkg_cmd_dir.join(format!("{}.md", cmd));
                let dest = opencode_cmd_dir.join(format!("{}.md", cmd));

                if dest.exists() {
                    opencode_already_exist += 1;
                    continue;
                }

                if source.exists() {
                    fs::copy(&source, &dest)?;
                    opencode_added += 1;
                }
            }
        }

        // Add hooks
        if let Some(ref pkg_hook_dir) = package_opencode_hook_dir {
            for hook in OPENCODE_HOOKS {
                let source = pkg_hook_dir.join(format!("{}.md", hook));
                let dest = opencode_hook_dir.join(format!("{}.md", hook));

                if dest.exists() {
                    opencode_already_exist += 1;
                    continue;
                }

                if source.exists() {
                    fs::copy(&source, &dest)?;
                    opencode_added += 1;
                }
            }
        }

        // Add tools
        if let Some(ref pkg_tool_dir) = package_opencode_tool_dir {
            for tool in OPENCODE_TOOLS {
                let source = pkg_tool_dir.join(format!("{}.json", tool));
                let dest = opencode_tool_dir.join(format!("{}.json", tool));

                if dest.exists() {
                    opencode_already_exist += 1;
                    continue;
                }

                if source.exists() {
                    fs::copy(&source, &dest)?;
                    opencode_added += 1;
                }
            }
        }

        if opencode_added > 0 {
            println!("  {} {} files installed", "✓".green(), opencode_added);
        }
        if opencode_already_exist > 0 {
            println!(
                "  {} {} files already installed",
                "·".yellow(),
                opencode_already_exist
            );
        }
    }

    println!();
    let total_added = agents_added + skills_added + opencode_added;
    let total_existing = agents_already_exist + skills_already_exist + opencode_already_exist;

    if total_added > 0 {
        println!(
            "{}",
            format!("✅ Added {} item(s)", total_added).green().bold()
        );
    }
    if total_existing > 0 {
        println!(
            "{}",
            format!("{} item(s) already installed", total_existing).yellow()
        );
    }

    Ok(())
}

/// Recursively remove a directory
fn remove_dir_recursive(path: &PathBuf) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

/// Remove SCUD agent(s), skill(s), and OpenCode integration
pub fn agents_remove(project_root: Option<PathBuf>, name: Option<String>, all: bool) -> Result<()> {
    if !all && name.is_none() {
        anyhow::bail!("Please specify an agent/skill name or use --all to remove all");
    }

    let scud_dir = get_scud_commands_dir(project_root.clone());
    let skills_dir = get_skills_dir(project_root.clone());
    let opencode_cmd_dir = get_opencode_command_dir(project_root.clone());
    let opencode_hook_dir = get_opencode_hook_dir(project_root.clone());
    let opencode_tool_dir = get_opencode_tool_dir(project_root.clone());
    let opencode_skills_dir = get_opencode_skills_dir(project_root);

    let mut agents_removed = 0;
    let mut agents_not_found = 0;
    let mut skills_removed = 0;
    let mut skills_not_found = 0;
    let mut opencode_removed = 0;

    // Determine what to remove
    let (agents_to_remove, skills_to_remove): (Vec<&str>, Vec<&str>) = if all {
        (
            SCUD_AGENTS
                .iter()
                .map(|(filename, _, _)| *filename)
                .collect(),
            SCUD_SKILLS.iter().map(|(dirname, _, _)| *dirname).collect(),
        )
    } else {
        let name_ref = name.as_ref().unwrap();
        // Try agent first, then skill
        if let Some(agent) = normalize_agent_name(name_ref) {
            (vec![agent], vec![])
        } else if let Some(skill) = normalize_skill_name(name_ref) {
            (vec![], vec![skill])
        } else {
            anyhow::bail!(
                "Unknown agent/skill: '{}'. Valid agents: pm, sm, architect, dev, retrospective, status. Valid skills: scud-tasks",
                name_ref
            );
        }
    };

    // Remove agents
    if !agents_to_remove.is_empty() {
        println!("{}", "Agents:".blue().bold());
        for agent_name in &agents_to_remove {
            let agent_file = scud_dir.join(format!("{}.md", agent_name));

            if !agent_file.exists() {
                agents_not_found += 1;
                println!("  {} {} (not installed)", "·".yellow(), agent_name);
                continue;
            }

            fs::remove_file(&agent_file)?;
            agents_removed += 1;
            println!("  {} {}", "✓".green(), agent_name);
        }
    }

    // Remove skills
    if !skills_to_remove.is_empty() {
        println!("{}", "Skills:".blue().bold());
        for skill_name in &skills_to_remove {
            let skill_dir = skills_dir.join(skill_name);

            if !skill_dir.exists() {
                skills_not_found += 1;
                println!("  {} {} (not installed)", "·".yellow(), skill_name);
                continue;
            }

            remove_dir_recursive(&skill_dir)?;
            skills_removed += 1;
            println!("  {} {}", "✓".green(), skill_name);

            // Also remove from OpenCode skills directory
            let opencode_skill = opencode_skills_dir.join(skill_name);
            if opencode_skill.exists() {
                remove_dir_recursive(&opencode_skill)?;
            }
        }
    }

    // Remove OpenCode integration (only when --all)
    if all {
        println!("{}", "OpenCode:".blue().bold());

        // Remove commands
        for cmd in OPENCODE_COMMANDS {
            let cmd_file = opencode_cmd_dir.join(format!("{}.md", cmd));
            if cmd_file.exists() {
                fs::remove_file(&cmd_file)?;
                opencode_removed += 1;
            }
        }

        // Remove hooks
        for hook in OPENCODE_HOOKS {
            let hook_file = opencode_hook_dir.join(format!("{}.md", hook));
            if hook_file.exists() {
                fs::remove_file(&hook_file)?;
                opencode_removed += 1;
            }
        }

        // Remove tools
        for tool in OPENCODE_TOOLS {
            let tool_file = opencode_tool_dir.join(format!("{}.json", tool));
            if tool_file.exists() {
                fs::remove_file(&tool_file)?;
                opencode_removed += 1;
            }
        }

        if opencode_removed > 0 {
            println!("  {} {} files removed", "✓".green(), opencode_removed);
        } else {
            println!("  {} Not installed", "·".yellow());
        }
    }

    println!();
    let total_removed = agents_removed + skills_removed + opencode_removed;
    let total_not_found = agents_not_found + skills_not_found;

    if total_removed > 0 {
        println!(
            "{}",
            format!("✅ Removed {} item(s)", total_removed)
                .green()
                .bold()
        );
    }
    if total_not_found > 0 {
        println!(
            "{}",
            format!("{} item(s) were not installed", total_not_found).yellow()
        );
    }

    Ok(())
}
