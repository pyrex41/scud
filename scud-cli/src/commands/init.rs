use anyhow::Result;
use colored::Colorize;
use dialoguer::{Input, Select};
use std::path::PathBuf;

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
            "xai" | "anthropic" | "openai" | "openrouter"
        ) {
            anyhow::bail!(
                "Invalid provider: {}. Valid options: xai, anthropic, openai, openrouter",
                provider
            );
        }
        let model = Config::default_model_for_provider(&provider).to_string();
        (provider, model)
    } else if is_interactive() {
        // Interactive mode - prompt for LLM provider
        let providers = vec![
            "xAI (Grok)",
            "Anthropic (Claude)",
            "OpenAI (GPT)",
            "OpenRouter",
        ];
        let provider_selection = Select::new()
            .with_prompt("Select your LLM provider")
            .items(&providers)
            .default(0)
            .interact()?;

        let provider = match provider_selection {
            0 => "xai",
            1 => "anthropic",
            2 => "openai",
            3 => "openrouter",
            _ => "anthropic",
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
        // Non-interactive without provider arg: use default (anthropic)
        let provider = "anthropic";
        let model = Config::default_model_for_provider(provider);
        (provider.to_string(), model.to_string())
    };

    let config = Config {
        llm: LLMConfig {
            provider,
            model,
            max_tokens: 4096,
        },
    };

    storage.initialize_with_config(&config)?;

    println!("\n{}", "✅ SCUD initialized successfully!".green().bold());
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
    println!("  3. Start with: /scud:pm (or use Claude Code slash command)\n");

    Ok(())
}
