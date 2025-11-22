use anyhow::Result;
use colored::Colorize;
use dialoguer::Select;
use std::path::PathBuf;

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
    } else {
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

        let (provider, model) = match provider_selection {
            0 => ("xai", Config::default_model_for_provider("xai")),
            1 => ("anthropic", Config::default_model_for_provider("anthropic")),
            2 => ("openai", Config::default_model_for_provider("openai")),
            3 => (
                "openrouter",
                Config::default_model_for_provider("openrouter"),
            ),
            _ => ("anthropic", Config::default_model_for_provider("anthropic")),
        };

        (provider.to_string(), model.to_string())
    };

    let config = Config {
        llm: LLMConfig {
            provider,
            model,
            max_tokens: 4096,
            research_model: None,
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
    println!("  3. Start with: /tm-pm (or use Claude Code slash command)\n");

    Ok(())
}
