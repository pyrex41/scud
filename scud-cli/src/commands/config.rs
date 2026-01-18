use anyhow::Result;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;
use crate::storage::Storage;

/// Embedded SCUD command definitions
/// Each command has a filename and content
/// Commands are stored in .claude/commands/scud/<filename>.md
const EMBEDDED_SCUD_COMMANDS: &[(&str, &str)] = &[
    ("stats", include_str!("../../assets/commands/scud/stats.md")),
    ("next", include_str!("../../assets/commands/scud/next.md")),
    ("show", include_str!("../../assets/commands/scud/show.md")),
    ("list", include_str!("../../assets/commands/scud/list.md")),
    ("waves", include_str!("../../assets/commands/scud/waves.md")),
    (
        "status",
        include_str!("../../assets/commands/scud/status.md"),
    ),
];

/// Embedded SCUD skill definitions
/// Skills are stored in .claude/skills/<skill-name>/SKILL.md
const EMBEDDED_SCUD_SKILLS: &[(&str, &str)] = &[
    (
        "scud-tasks",
        include_str!("../../assets/skills/scud-tasks/SKILL.md"),
    ),
    ("scud", include_str!("../../assets/skills/scud/SKILL.md")),
];

/// Embedded SCUD spawn agent definitions
/// Agent definitions for model routing (stored in .scud/agents/<name>.toml)
const EMBEDDED_SPAWN_AGENTS: &[(&str, &str)] = &[
    (
        "builder",
        include_str!("../assets/spawn-agents/builder.toml"),
    ),
    (
        "reviewer",
        include_str!("../assets/spawn-agents/reviewer.toml"),
    ),
    (
        "planner",
        include_str!("../assets/spawn-agents/planner.toml"),
    ),
    (
        "researcher",
        include_str!("../assets/spawn-agents/researcher.toml"),
    ),
    (
        "analyzer",
        include_str!("../assets/spawn-agents/analyzer.toml"),
    ),
    (
        "fast-builder",
        include_str!("../assets/spawn-agents/fast-builder.toml"),
    ),
    (
        "outside-generalist",
        include_str!("../assets/spawn-agents/outside-generalist.toml"),
    ),
    (
        "repairer",
        include_str!("../assets/spawn-agents/repairer.toml"),
    ),
];

/// SCUD agent definitions (legacy - keeping for compatibility)
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
const SCUD_SKILLS: &[(&str, &[&str], &str)] = &[
    (
        "scud-tasks",
        &["scud-tasks", "tasks"],
        "Task management - view, update, claim, and track tasks",
    ),
    (
        "scud",
        &["scud", "guide"],
        "SCUD CLI usage guide - list, waves, tags, next, log, etc",
    ),
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

// Package directory functions removed - now using embedded files

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

    // Check commands
    for cmd in OPENCODE_COMMANDS {
        let cmd_file = opencode_cmd_dir.join(format!("{}.md", cmd));
        if cmd_file.exists() {
            opencode_installed += 1;
        }
    }

    // Check hooks
    for hook in OPENCODE_HOOKS {
        let hook_file = opencode_hook_dir.join(format!("{}.md", hook));
        if hook_file.exists() {
            opencode_installed += 1;
        }
    }

    // Check tools
    for tool in OPENCODE_TOOLS {
        let tool_file = opencode_tool_dir.join(format!("{}.json", tool));
        if tool_file.exists() {
            opencode_installed += 1;
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

/// Add SCUD agent(s), skill(s), and OpenCode integration
pub fn agents_add(project_root: Option<PathBuf>, name: Option<String>, all: bool) -> Result<()> {
    if !all && name.is_none() {
        anyhow::bail!("Please specify an agent/skill name or use --all to add all");
    }

    // No longer need package directories - using embedded files
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
            EMBEDDED_SCUD_COMMANDS
                .iter()
                .map(|(name, _)| *name)
                .collect(),
            EMBEDDED_SCUD_SKILLS.iter().map(|(name, _)| *name).collect(),
        )
    } else {
        let name_ref = name.as_ref().unwrap();
        // Try agent first, then skill
        if EMBEDDED_SCUD_COMMANDS.iter().any(|(n, _)| *n == name_ref) {
            (vec![name_ref], vec![])
        } else if EMBEDDED_SCUD_SKILLS.iter().any(|(n, _)| *n == name_ref) {
            (vec![], vec![name_ref])
        } else {
            anyhow::bail!(
                "Unknown agent/skill: '{}'. Valid agents: {}. Valid skills: {}",
                name_ref,
                EMBEDDED_SCUD_COMMANDS
                    .iter()
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", "),
                EMBEDDED_SCUD_SKILLS
                    .iter()
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    };

    // Add agents (using embedded content)
    if !agents_to_add.is_empty() {
        println!("{}", "Agents:".blue().bold());
        for agent_name in &agents_to_add {
            let dest = scud_dir.join(format!("{}.md", agent_name));

            if dest.exists() {
                agents_already_exist += 1;
                println!("  {} {} (already installed)", "·".yellow(), agent_name);
                continue;
            }

            // Find embedded content
            if let Some((_, content)) = EMBEDDED_SCUD_COMMANDS
                .iter()
                .find(|(n, _)| *n == *agent_name)
            {
                fs::write(&dest, content)?;
                agents_added += 1;
                println!("  {} {}", "✓".green(), agent_name.green());
            } else {
                println!(
                    "  {} {} (embedded content not found)",
                    "✗".red(),
                    agent_name
                );
            }
        }
    }

    // Add skills (using embedded content)
    if !skills_to_add.is_empty() {
        println!("{}", "Skills:".blue().bold());
        for skill_name in &skills_to_add {
            let dest = skills_dir.join(skill_name);
            let skill_file = dest.join("SKILL.md");

            if skill_file.exists() {
                skills_already_exist += 1;
                println!("  {} {} (already installed)", "·".yellow(), skill_name);
                continue;
            }

            // Find embedded content
            if let Some((_, content)) = EMBEDDED_SCUD_SKILLS.iter().find(|(n, _)| *n == *skill_name)
            {
                fs::create_dir_all(&dest)?;
                fs::write(&skill_file, content)?;
                skills_added += 1;
                println!("  {} {}", "✓".green(), skill_name.green());

                // Also copy skill to OpenCode skills directory
                let opencode_dest = opencode_skills_dir.join(skill_name);
                let opencode_skill_file = opencode_dest.join("SKILL.md");
                if !opencode_skill_file.exists() {
                    fs::create_dir_all(&opencode_dest)?;
                    fs::write(&opencode_skill_file, content)?;
                }
            } else {
                println!(
                    "  {} {} (embedded content not found)",
                    "✗".red(),
                    skill_name
                );
            }
        }
    }

    // Add OpenCode integration (only when --all) - using embedded SCUD commands
    if all {
        println!("{}", "OpenCode:".blue().bold());

        // Ensure OpenCode directories exist
        fs::create_dir_all(&opencode_cmd_dir)?;
        fs::create_dir_all(&opencode_hook_dir)?;
        fs::create_dir_all(&opencode_tool_dir)?;

        // Add commands (using embedded SCUD commands)
        for cmd in OPENCODE_COMMANDS {
            let dest = opencode_cmd_dir.join(format!("{}.md", cmd));

            if dest.exists() {
                opencode_already_exist += 1;
                continue;
            }

            // Map OpenCode command names to embedded SCUD commands
            let embedded_name = match *cmd {
                "task-list" => "list",
                "task-next" => "next",
                "task-show" => "show",
                "task-status" => "status",
                "task-claim" => "status",   // closest match
                "task-release" => "status", // closest match
                "task-waves" => "waves",
                "task-stats" => "stats",
                "task-whois" => "status",  // closest match
                "task-tags" => "status",   // closest match
                "task-doctor" => "status", // closest match
                _ => continue,
            };

            if let Some((_, content)) = EMBEDDED_SCUD_COMMANDS
                .iter()
                .find(|(n, _)| *n == embedded_name)
            {
                fs::write(&dest, content)?;
                opencode_added += 1;
            }
        }

        // Add hooks (simplified - just create empty hook files for now)
        for hook in OPENCODE_HOOKS {
            let dest = opencode_hook_dir.join(format!("{}.md", hook));

            if dest.exists() {
                opencode_already_exist += 1;
                continue;
            }

            // Create basic hook content
            let hook_content = "# Session Start Hook\n\nThis hook runs when an OpenCode session starts.\n\n```bash\nscud warmup\n```".to_string();
            fs::write(&dest, hook_content)?;
            opencode_added += 1;
        }

        // Add tools (create basic tool definitions)
        for tool in OPENCODE_TOOLS {
            let dest = opencode_tool_dir.join(format!("{}.json", tool));

            if dest.exists() {
                opencode_already_exist += 1;
                continue;
            }

            // Create basic tool definitions
            let tool_content = match *tool {
                "find_skills" => {
                    r#"{
  "name": "find_skills",
  "description": "Find available skills in the codebase",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Search query for skills"
      }
    }
  }
}"#
                }
                "use_skill" => {
                    r#"{
  "name": "use_skill",
  "description": "Use a specific skill",
  "inputSchema": {
    "type": "object",
    "properties": {
      "skill_name": {
        "type": "string",
        "description": "Name of the skill to use"
      },
      "parameters": {
        "type": "object",
        "description": "Parameters for the skill"
      }
    }
  }
}"#
                }
                _ => continue,
            };

            fs::write(&dest, tool_content)?;
            opencode_added += 1;
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

/// Configure backpressure validation commands
pub fn backpressure(
    project_root: Option<PathBuf>,
    commands: Vec<String>,
    add: Option<String>,
    remove: Option<String>,
    list: bool,
    clear: bool,
) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD is not initialized. Run: scud init");
    }

    let config_path = storage.config_file();

    // Load existing config
    let content = fs::read_to_string(&config_path).unwrap_or_default();
    let mut config: toml::Value =
        toml::from_str(&content).unwrap_or(toml::Value::Table(toml::map::Map::new()));

    // Get or create swarm.backpressure section
    let bp_commands = get_backpressure_commands(&config);

    if list {
        // List current configuration
        println!("{}", "Backpressure Configuration".blue().bold());
        println!();

        if bp_commands.is_empty() {
            println!(
                "  {} No commands configured (using auto-detect)",
                "·".yellow()
            );
            println!();

            // Show what would be auto-detected
            let auto = crate::backpressure::BackpressureConfig::load(Some(
                &storage.project_root().to_path_buf(),
            ))?;
            if !auto.commands.is_empty() {
                println!("{}", "Auto-detected commands:".dimmed());
                for cmd in &auto.commands {
                    println!("  {} {}", "·".dimmed(), cmd.dimmed());
                }
            }
        } else {
            println!("{}", "Commands (in order):".blue());
            for (i, cmd) in bp_commands.iter().enumerate() {
                println!("  {}. {}", i + 1, cmd.green());
            }
        }

        println!();
        println!("{}", "Usage:".blue().bold());
        println!("  scud config backpressure \"cmd1\" \"cmd2\"   Set commands");
        println!("  scud config backpressure --add \"cmd\"     Add a command");
        println!("  scud config backpressure --remove \"cmd\"  Remove a command");
        println!("  scud config backpressure --clear         Clear (use auto-detect)");

        return Ok(());
    }

    if clear {
        // Remove backpressure section entirely
        if let Some(swarm) = config.get_mut("swarm") {
            if let Some(table) = swarm.as_table_mut() {
                table.remove("backpressure");
            }
        }
        save_config(&config_path, &config)?;
        println!(
            "{}",
            "✓ Backpressure config cleared (will use auto-detect)".green()
        );
        return Ok(());
    }

    let mut new_commands = bp_commands.clone();

    if let Some(cmd) = add {
        if !new_commands.contains(&cmd) {
            new_commands.push(cmd.clone());
            println!("{}", format!("✓ Added: {}", cmd).green());
        } else {
            println!("{}", format!("· Already exists: {}", cmd).yellow());
        }
    } else if let Some(cmd) = remove {
        if let Some(pos) = new_commands.iter().position(|c| c == &cmd) {
            new_commands.remove(pos);
            println!("{}", format!("✓ Removed: {}", cmd).green());
        } else {
            println!("{}", format!("· Not found: {}", cmd).yellow());
        }
    } else if !commands.is_empty() {
        // Set commands directly
        new_commands = commands;
        println!("{}", "✓ Backpressure commands set:".green());
        for cmd in &new_commands {
            println!("  · {}", cmd);
        }
    } else {
        // No args - show list
        return backpressure(
            Some(storage.project_root().to_path_buf()),
            vec![],
            None,
            None,
            true,
            false,
        );
    }

    // Save updated config
    set_backpressure_commands(&mut config, &new_commands);
    save_config(&config_path, &config)?;

    Ok(())
}

/// Get backpressure commands from config
fn get_backpressure_commands(config: &toml::Value) -> Vec<String> {
    config
        .get("swarm")
        .and_then(|s| s.get("backpressure"))
        .and_then(|b| b.get("commands"))
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Set backpressure commands in config
fn set_backpressure_commands(config: &mut toml::Value, commands: &[String]) {
    let table = config.as_table_mut().expect("Config must be a table");

    // Ensure swarm section exists
    if !table.contains_key("swarm") {
        table.insert(
            "swarm".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );
    }

    let swarm = table.get_mut("swarm").unwrap().as_table_mut().unwrap();

    // Ensure backpressure section exists
    if !swarm.contains_key("backpressure") {
        swarm.insert(
            "backpressure".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );
    }

    let bp = swarm
        .get_mut("backpressure")
        .unwrap()
        .as_table_mut()
        .unwrap();

    // Set commands array
    let cmd_array: Vec<toml::Value> = commands
        .iter()
        .map(|s| toml::Value::String(s.clone()))
        .collect();
    bp.insert("commands".to_string(), toml::Value::Array(cmd_array));

    // Ensure defaults exist
    if !bp.contains_key("stop_on_failure") {
        bp.insert("stop_on_failure".to_string(), toml::Value::Boolean(true));
    }
    if !bp.contains_key("timeout_secs") {
        bp.insert("timeout_secs".to_string(), toml::Value::Integer(300));
    }
}

/// Save config to file
fn save_config(path: &PathBuf, config: &toml::Value) -> Result<()> {
    let content = toml::to_string_pretty(config)?;
    fs::write(path, content)?;
    Ok(())
}

/// Get the .scud/agents directory for spawn agent definitions
fn get_spawn_agents_dir(project_root: Option<PathBuf>) -> PathBuf {
    let base = project_root.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    base.join(".scud").join("agents")
}

/// Install spawn agent definitions to .scud/agents/
/// These define harness/model routing for different agent types
pub fn spawn_agents_add(
    project_root: Option<PathBuf>,
    name: Option<String>,
    all: bool,
    interactive: bool,
) -> Result<()> {
    let agents_dir = get_spawn_agents_dir(project_root);
    fs::create_dir_all(&agents_dir)?;

    let agents_to_add: Vec<&str> = if all {
        EMBEDDED_SPAWN_AGENTS.iter().map(|(n, _)| *n).collect()
    } else if let Some(ref name) = name {
        if EMBEDDED_SPAWN_AGENTS
            .iter()
            .any(|(n, _)| *n == name.as_str())
        {
            vec![name.as_str()]
        } else {
            anyhow::bail!(
                "Unknown spawn agent: '{}'. Available: {}",
                name,
                EMBEDDED_SPAWN_AGENTS
                    .iter()
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    } else if interactive {
        // Interactive selection
        use dialoguer::MultiSelect;

        let items: Vec<String> = EMBEDDED_SPAWN_AGENTS
            .iter()
            .map(|(name, content)| {
                // Extract description from toml
                let desc = content
                    .lines()
                    .find(|l| l.starts_with("description"))
                    .and_then(|l| l.split('=').nth(1))
                    .map(|s| s.trim().trim_matches('"'))
                    .unwrap_or("");
                format!("{} - {}", name, desc)
            })
            .collect();

        let selections = MultiSelect::new()
            .with_prompt("Select spawn agents to install (space to toggle)")
            .items(&items)
            .defaults(&vec![true; items.len()])
            .interact()?;

        selections
            .iter()
            .map(|&i| EMBEDDED_SPAWN_AGENTS[i].0)
            .collect()
    } else {
        anyhow::bail!("Please specify an agent name, use --all, or run interactively");
    };

    if agents_to_add.is_empty() {
        println!("{}", "No agents selected.".yellow());
        return Ok(());
    }

    println!("{}", "Spawn Agents:".blue().bold());
    let mut added = 0;
    let mut existing = 0;

    for agent_name in agents_to_add {
        let dest = agents_dir.join(format!("{}.toml", agent_name));

        if dest.exists() {
            existing += 1;
            println!("  {} {} (already installed)", "·".yellow(), agent_name);
            continue;
        }

        if let Some((_, content)) = EMBEDDED_SPAWN_AGENTS.iter().find(|(n, _)| *n == agent_name) {
            fs::write(&dest, content)?;
            added += 1;
            println!("  {} {}", "✓".green(), agent_name.green());
        }
    }

    println!();
    if added > 0 {
        println!(
            "{}",
            format!("✅ Installed {} spawn agent(s)", added)
                .green()
                .bold()
        );
        println!(
            "{}",
            "Agents are used via @agents section in .scg files".dimmed()
        );
    }
    if existing > 0 {
        println!(
            "{}",
            format!("{} agent(s) already installed", existing).yellow()
        );
    }

    Ok(())
}

/// List available spawn agents
pub fn spawn_agents_list(project_root: Option<PathBuf>) -> Result<()> {
    let agents_dir = get_spawn_agents_dir(project_root);

    println!("{}", "Available Spawn Agents:".blue().bold());
    println!();

    for (name, content) in EMBEDDED_SPAWN_AGENTS {
        let installed = agents_dir.join(format!("{}.toml", name)).exists();
        let status = if installed {
            "✓".green()
        } else {
            "·".dimmed()
        };

        // Extract description and model info from toml
        let desc = content
            .lines()
            .find(|l| l.starts_with("description"))
            .and_then(|l| l.split('=').nth(1))
            .map(|s| s.trim().trim_matches('"'))
            .unwrap_or("");

        let harness = content
            .lines()
            .find(|l| l.starts_with("harness"))
            .and_then(|l| l.split('=').nth(1))
            .map(|s| s.trim().trim_matches('"'))
            .unwrap_or("?");

        let model = content
            .lines()
            .find(|l| l.trim().starts_with("model") && !l.contains('['))
            .and_then(|l| l.split('=').nth(1))
            .map(|s| s.trim().trim_matches('"'))
            .unwrap_or("default");

        println!(
            "  {} {:<14} [{}:{}] {}",
            status,
            name.cyan(),
            harness,
            model,
            desc.dimmed()
        );
    }

    println!();
    println!("Install: {}", "scud config spawn-agents add --all".cyan());

    Ok(())
}

/// Remove spawn agent definitions
pub fn spawn_agents_remove(
    project_root: Option<PathBuf>,
    name: Option<String>,
    all: bool,
) -> Result<()> {
    let agents_dir = get_spawn_agents_dir(project_root);

    let agents_to_remove: Vec<&str> = if all {
        EMBEDDED_SPAWN_AGENTS.iter().map(|(n, _)| *n).collect()
    } else if let Some(ref name) = name {
        vec![name.as_str()]
    } else {
        anyhow::bail!("Please specify an agent name or use --all");
    };

    println!("{}", "Removing Spawn Agents:".blue().bold());
    let mut removed = 0;
    let mut not_found = 0;

    for agent_name in agents_to_remove {
        let path = agents_dir.join(format!("{}.toml", agent_name));

        if !path.exists() {
            not_found += 1;
            println!("  {} {} (not installed)", "·".yellow(), agent_name);
            continue;
        }

        fs::remove_file(&path)?;
        removed += 1;
        println!("  {} {}", "✓".green(), agent_name);
    }

    println!();
    if removed > 0 {
        println!(
            "{}",
            format!("✅ Removed {} spawn agent(s)", removed)
                .green()
                .bold()
        );
    }
    if not_found > 0 {
        println!(
            "{}",
            format!("{} agent(s) were not installed", not_found).yellow()
        );
    }

    Ok(())
}
