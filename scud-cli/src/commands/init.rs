use anyhow::Result;
use colored::Colorize;
use dialoguer::{Input, Select};
use std::fs;
use std::path::PathBuf;

use crate::commands::config as config_cmd;
use crate::commands::helpers::is_interactive;
use crate::config::{Config, LLMConfig};
use crate::storage::Storage;

pub fn run(project_root: Option<PathBuf>, provider_arg: Option<String>) -> Result<()> {
    let storage = Storage::new(project_root);

    if storage.is_initialized() {
        println!("{}", "✓ SCUD is already initialized".green());
        return Ok(());
    }

    println!("{}", "Initializing SCUD...".blue());
    println!();

    let (provider, model) = if let Some(provider_name) = provider_arg {
        // Non-interactive mode with command-line argument
        let provider = provider_name.to_lowercase();
        if !matches!(
            provider.as_str(),
            "xai" | "anthropic" | "openai" | "openrouter" | "claude-cli"
        ) {
            anyhow::bail!(
                "Invalid provider: {}. Valid options: claude-cli, xai, anthropic, openai, openrouter",
                provider
            );
        }
        let model = Config::default_model_for_provider(&provider).to_string();
        (provider, model)
    } else if is_interactive() {
        // Interactive mode - prompt for LLM provider
        let providers = vec![
            "Claude Code (recommended - no API key needed)",
            "xAI (Grok)",
            "Anthropic (Claude API)",
            "OpenAI (GPT)",
            "OpenRouter",
        ];
        let provider_selection = Select::new()
            .with_prompt("Select your LLM provider")
            .items(&providers)
            .default(0)
            .interact()?;

        let provider = match provider_selection {
            0 => "claude-cli",
            1 => "xai",
            2 => "anthropic",
            3 => "openai",
            4 => "openrouter",
            _ => "claude-cli",
        };

        // Build model options: suggested models + "Custom" option
        let suggested = Config::suggested_models_for_provider(provider);
        let mut model_options: Vec<String> = suggested.iter().map(|s| s.to_string()).collect();
        model_options.push("Custom (enter model name)".to_string());

        let model_selection = Select::new()
            .with_prompt("Select model (or choose Custom to enter any model)")
            .items(&model_options)
            .default(0)
            .interact()?;

        let model = if model_selection == model_options.len() - 1 {
            // User selected "Custom"
            Input::<String>::new()
                .with_prompt("Enter model name")
                .interact_text()?
        } else {
            suggested[model_selection].to_string()
        };

        (provider.to_string(), model)
    } else {
        // Non-interactive without provider arg: use default (claude-cli)
        let provider = "claude-cli";
        let model = Config::default_model_for_provider(provider);
        (provider.to_string(), model.to_string())
    };

    // Determine smart/fast models based on provider
    let (smart_model, fast_model) = match provider.as_str() {
        "claude-cli" => ("opus".to_string(), "sonnet".to_string()),
        "anthropic" => (
            "claude-opus-4-5-20251101".to_string(),
            "claude-sonnet-4-5-20250929".to_string(),
        ),
        "xai" => (
            "grok-4-1-fast-reasoning".to_string(),
            "grok-code-fast-1".to_string(),
        ),
        "openai" => ("o3".to_string(), "gpt-5.1-mini".to_string()),
        "openrouter" => (
            "anthropic/claude-opus-4.5".to_string(),
            "anthropic/claude-sonnet-4.5".to_string(),
        ),
        _ => ("opus".to_string(), "sonnet".to_string()),
    };

    let config = Config {
        llm: LLMConfig {
            provider,
            model,
            smart_model,
            fast_model,
            max_tokens: 16000,
        },
    };

    storage.initialize_with_config(&config)?;

    println!("\n{}", "SCUD initialized successfully!".green().bold());

    // Auto-install all agents and commands
    println!("\n{}", "Installing SCUD agents and commands...".blue());
    if let Err(e) = config_cmd::agents_add(Some(storage.project_root().to_path_buf()), None, true) {
        println!(
            "{}",
            format!("  Could not install agents: {}", e).yellow()
        );
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
    println!("  Provider: {}", config.llm.provider.yellow());
    println!("  Model: {}", config.llm.model.yellow());
    println!("\n{}", "Environment variable required:".blue());
    println!(
        "  export {}=your-api-key",
        config.api_key_env_var().yellow()
    );
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
