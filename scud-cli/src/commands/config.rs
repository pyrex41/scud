use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::config::Config;
use crate::storage::Storage;

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
    if let Some(research_model) = &config.llm.research_model {
        println!("  {}: {}", "Research Model".yellow(), research_model);
    }
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

pub fn set_research_model(project_root: Option<PathBuf>, model: Option<String>) -> Result<()> {
    let storage = Storage::new(project_root);

    if !storage.is_initialized() {
        anyhow::bail!("SCUD is not initialized. Run: scud init");
    }

    let mut config = storage.load_config()?;

    match model {
        Some(m) => {
            config.llm.research_model = Some(m.clone());
            config.save(&storage.config_file())?;

            println!("{}", "✅ Research model updated!".green().bold());
            println!();
            println!("  {}: {}", "Research Model".yellow(), m);
            println!();
            println!("{}", "Research tasks will now use this model.".blue());
        }
        None => {
            config.llm.research_model = None;
            config.save(&storage.config_file())?;

            println!("{}", "✅ Research model cleared!".green().bold());
            println!();
            println!("{}", "Research tasks will use the main model.".blue());
        }
    }

    Ok(())
}
