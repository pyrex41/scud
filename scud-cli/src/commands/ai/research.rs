use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

use crate::llm::{LLMClient, Prompts};
use crate::storage::Storage;

pub async fn run(project_root: Option<PathBuf>, query: &str) -> Result<()> {
    let client = LLMClient::new()?;

    // Load config to check for research-specific model
    let storage = Storage::new(project_root);
    let config = storage.load_config()?;
    let research_model = config.llm.research_model.as_deref();

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );

    let model_info = if let Some(model) = research_model {
        format!(" (using {})", model)
    } else {
        String::new()
    };
    spinner.set_message(format!("Researching{}: {}", model_info, query));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let prompt = Prompts::research_topic(query);
    let response = client.complete_with_model(&prompt, research_model).await?;

    spinner.finish_and_clear();

    println!("\n{}", "Research Results".blue().bold());
    println!("{}", "================".blue());
    println!("{}: {}", "Query".yellow(), query);
    println!();
    println!("{}", response);
    println!();

    Ok(())
}
