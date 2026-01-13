use anyhow::Result;
use colored::Colorize;
use dialoguer::{Input, Select};
use std::fs;
use std::path::PathBuf;

use crate::commands::config as config_cmd;
use crate::commands::helpers::is_interactive;
use crate::config::{Config, LLMConfig};
use crate::storage::Storage;

/// Helper function to configure provider and model for a specific tier
fn configure_provider_and_model(tier: &str) -> Result<(String, String)> {
    let providers = vec![
        "Claude Code (recommended - no API key needed)",
        "OpenAI Codex CLI (no API key needed)",
        "xAI (Grok)",
        "Anthropic (Claude API)",
        "OpenAI (GPT API)",
        "OpenRouter",
    ];
    let provider_selection = Select::new()
        .with_prompt(&format!("Select {} LLM provider", tier))
        .items(&providers)
        .default(if tier == "fast" { 2 } else { 0 }) // Default xAI for fast, Claude for smart
        .interact()?;

    let provider = match provider_selection {
        0 => "claude-cli",
        1 => "codex",
        2 => "xai",
        3 => "anthropic",
        4 => "openai",
        5 => "openrouter",
        _ => "claude-cli",
    };

    // Build model options: suggested models + "Custom" option
    let suggested = Config::suggested_models_for_provider(provider);
    let mut model_options: Vec<String> = suggested.iter().map(|s| s.to_string()).collect();
    model_options.push("Custom (enter model name)".to_string());

    let default_model_index = if tier == "fast" && provider == "xai" {
        suggested
            .iter()
            .position(|m| *m == "grok-code-fast-1")
            .unwrap_or(0)
    } else if tier == "smart" && provider == "claude-cli" {
        suggested.iter().position(|m| *m == "opus").unwrap_or(0)
    } else {
        0
    };

    let model_selection = Select::new()
        .with_prompt(&format!(
            "Select {} model (or choose Custom to enter any model)",
            tier
        ))
        .items(&model_options)
        .default(default_model_index)
        .interact()?;

    let model = if model_selection == model_options.len() - 1 {
        // User selected "Custom"
        Input::<String>::new()
            .with_prompt("Enter model name")
            .interact_text()?
    } else {
        suggested[model_selection].to_string()
    };

    Ok((provider.to_string(), model))
}

pub fn run(project_root: Option<PathBuf>, provider_arg: Option<String>) -> Result<()> {
    let storage = Storage::new(project_root);

    if storage.is_initialized() {
        println!("{}", "✓ SCUD is already initialized".green());
        return Ok(());
    }

    println!("{}", "Initializing SCUD...".blue());
    println!();

    let (provider, model, smart_provider, smart_model, fast_provider, fast_model) = if let Some(
        provider_name,
    ) =
        provider_arg
    {
        // Non-interactive mode with command-line argument - use defaults for all tiers
        let provider = provider_name.to_lowercase();
        if !matches!(
            provider.as_str(),
            "xai" | "anthropic" | "openai" | "openrouter" | "claude-cli" | "codex"
        ) {
            anyhow::bail!(
                "Invalid provider: {}. Valid options: claude-cli, codex, xai, anthropic, openai, openrouter",
                provider
            );
        }
        let model = Config::default_model_for_provider(&provider).to_string();
        // Use defaults for smart/fast (could be customized later)
        let smart_provider = "claude-cli".to_string();
        let smart_model = "opus".to_string();
        let fast_provider = "xai".to_string();
        let fast_model = "grok-code-fast-1".to_string();
        (
            provider,
            model,
            smart_provider,
            smart_model,
            fast_provider,
            fast_model,
        )
    } else if is_interactive() {
        println!(
            "{}",
            "SCUD supports separate models for different types of tasks:".blue()
        );
        println!("  • Fast models: Quick coding, generation tasks");
        println!("  • Smart models: Complex reasoning, analysis, validation");
        println!();

        // Configure FAST model/provider
        println!("{}", "=== FAST MODEL CONFIGURATION ===".yellow().bold());
        let (fast_provider, fast_model) = configure_provider_and_model("fast")?;

        // Configure SMART model/provider
        println!();
        println!("{}", "=== SMART MODEL CONFIGURATION ===".yellow().bold());
        let (smart_provider, smart_model) = configure_provider_and_model("smart")?;

        // Use fast provider/model as defaults for backward compatibility
        let provider = fast_provider.clone();
        let model = fast_model.clone();

        (
            provider,
            model,
            smart_provider,
            smart_model,
            fast_provider,
            fast_model,
        )
    } else {
        // Non-interactive without provider arg: use default (claude-cli)
        let provider = "claude-cli";
        let model = Config::default_model_for_provider(provider);
        // Use defaults for smart/fast
        let smart_provider = "claude-cli".to_string();
        let smart_model = "opus".to_string();
        let fast_provider = "xai".to_string();
        let fast_model = "grok-code-fast-1".to_string();
        (
            provider.to_string(),
            model.to_string(),
            smart_provider,
            smart_model,
            fast_provider,
            fast_model,
        )
    };

    let config = Config {
        llm: LLMConfig {
            provider,
            model,
            smart_provider,
            smart_model,
            fast_provider,
            fast_model,
            max_tokens: 16000,
        },
    };

    storage.initialize_with_config(&config)?;

    println!("\n{}", "SCUD initialized successfully!".green().bold());

    // Auto-install all agents and commands
    println!("\n{}", "Installing SCUD agents and commands...".blue());
    if let Err(e) = config_cmd::agents_add(Some(storage.project_root().to_path_buf()), None, true) {
        println!("{}", format!("  Could not install agents: {}", e).yellow());
        println!("  You can install them later with: scud config agents add --all");
    }

    // Update CLAUDE.md with SCUD instructions
    if let Err(e) = update_claude_md(&storage) {
        println!(
            "{}",
            format!("  Could not update CLAUDE.md: {}", e).yellow()
        );
    }

    println!("\n{}", "Configuration:".blue());
    println!(
        "  Default Provider: {} ({})",
        config.llm.provider.yellow(),
        config.llm.model.yellow()
    );
    println!(
        "  Fast Provider: {} ({})",
        config.llm.fast_provider.yellow(),
        config.llm.fast_model.yellow()
    );
    println!(
        "  Smart Provider: {} ({})",
        config.llm.smart_provider.yellow(),
        config.llm.smart_model.yellow()
    );
    if config.requires_api_key() {
        println!("\n{}", "Environment variables required:".blue());
        let mut env_vars = std::collections::HashSet::new();
        env_vars.insert(config.api_key_env_var());
        if config.llm.fast_provider != config.llm.provider {
            env_vars.insert(Config::api_key_env_var_for_provider(
                &config.llm.fast_provider,
            ));
        }
        if config.llm.smart_provider != config.llm.provider
            && config.llm.smart_provider != config.llm.fast_provider
        {
            env_vars.insert(Config::api_key_env_var_for_provider(
                &config.llm.smart_provider,
            ));
        }
        for env_var in env_vars {
            if env_var != "NONE" {
                println!("  export {}=your-api-key", env_var.yellow());
            }
        }
    } else {
        println!("\n{}", "No API keys required (using CLI tools)".green());
    }
    println!("\n{}", "Next steps:".blue());
    println!("  1. Set your API key environment variable");
    println!("  2. Run: scud tags");
    println!("  3. Create or import tasks, then use: /scud:next\n");

    Ok(())
}

/// Update CLAUDE.md with SCUD instructions
fn update_claude_md(storage: &Storage) -> Result<()> {
    let claude_md_path = storage.project_root().join("CLAUDE.md");

    let scud_section = r#"
## SCUD Task Management

This project uses SCUD for AI-driven task management.

### Quick Start
- `scud tags` - List available phases
- `scud next` - Find next available task
- `scud set-status <id> in-progress` - Claim a task
- `scud view` - Open interactive task viewer

### Slash Commands
Use `/scud:` commands in Claude Code for task operations.
"#;

    let marker = "## SCUD Task Management";

    if claude_md_path.exists() {
        let content = fs::read_to_string(&claude_md_path)?;
        if content.contains(marker) {
            return Ok(()); // Already has SCUD section
        }
        // Append to existing file
        let new_content = format!("{}\n{}", content.trim_end(), scud_section);
        fs::write(&claude_md_path, new_content)?;
    } else {
        // Create new file
        fs::write(&claude_md_path, scud_section.trim_start())?;
    }

    println!("  {} Updated CLAUDE.md with SCUD instructions", "✓".green());
    Ok(())
}
