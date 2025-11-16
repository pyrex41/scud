use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

use crate::llm::{LLMClient, Prompts};

pub async fn run(_project_root: Option<PathBuf>, query: &str) -> Result<()> {
    let client = LLMClient::new()?;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("Researching: {}", query));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let prompt = Prompts::research_topic(query);
    let response = client.complete(&prompt).await?;

    spinner.finish_and_clear();

    println!("\n{}", "Research Results".blue().bold());
    println!("{}", "================".blue());
    println!("{}: {}", "Query".yellow(), query);
    println!();
    println!("{}", response);
    println!();

    Ok(())
}
