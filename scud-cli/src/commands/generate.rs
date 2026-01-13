//! Generate command - combines parse, expand, and check-deps into a single pipeline.

use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};

use crate::commands::{ai, check_deps};

/// Options for the task generation pipeline.
///
/// This struct configures the multi-phase task generation process:
/// 1. **Parse**: Convert a PRD document into initial tasks
/// 2. **Expand**: Break down complex tasks into subtasks
/// 3. **Check Dependencies**: Validate and fix task dependencies
///
/// # Example
///
/// ```no_run
/// use scud::commands::generate::{generate, GenerateOptions};
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let options = GenerateOptions::new(
///         PathBuf::from("docs/prd.md"),
///         "my-feature".to_string(),
///     );
///
///     generate(options).await?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Project root directory (None for current directory)
    pub project_root: Option<PathBuf>,
    /// Path to the PRD/spec document to parse
    pub file: PathBuf,
    /// Tag name for generated tasks
    pub tag: String,
    /// Number of tasks to generate (default: 10)
    pub num_tasks: u32,
    /// Skip task expansion phase
    pub no_expand: bool,
    /// Skip dependency validation phase
    pub no_check_deps: bool,
    /// Append tasks to existing tag instead of replacing
    pub append: bool,
    /// Skip loading guidance from .scud/guidance/
    pub no_guidance: bool,
    /// Task ID format: "sequential" (default) or "uuid"
    pub id_format: String,
    /// Model to use for AI operations (overrides config)
    pub model: Option<String>,
    /// Show what would be done without making changes
    pub dry_run: bool,
    /// Verbose output showing each phase's details
    pub verbose: bool,
}

impl GenerateOptions {
    /// Create new options with required fields and sensible defaults.
    ///
    /// # Arguments
    ///
    /// * `file` - Path to the PRD/spec document
    /// * `tag` - Tag name for the generated tasks
    pub fn new(file: PathBuf, tag: String) -> Self {
        Self {
            project_root: None,
            file,
            tag,
            num_tasks: 10,
            no_expand: false,
            no_check_deps: false,
            append: false,
            no_guidance: false,
            id_format: "sequential".to_string(),
            model: None,
            dry_run: false,
            verbose: false,
        }
    }
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            project_root: None,
            file: PathBuf::new(),
            tag: String::new(),
            num_tasks: 10,
            no_expand: false,
            no_check_deps: false,
            append: false,
            no_guidance: false,
            id_format: "sequential".to_string(),
            model: None,
            dry_run: false,
            verbose: false,
        }
    }
}

/// Run the task generation pipeline with the given options.
///
/// This is the main entry point for programmatic task generation.
/// It orchestrates the parse → expand → check-deps pipeline.
///
/// # Example
///
/// ```no_run
/// use scud::commands::generate::{generate, GenerateOptions};
/// use std::path::PathBuf;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let mut options = GenerateOptions::new(
///         PathBuf::from("requirements.md"),
///         "api".to_string(),
///     );
///     options.num_tasks = 15;
///     options.verbose = true;
///
///     generate(options).await?;
///     Ok(())
/// }
/// ```
pub async fn generate(options: GenerateOptions) -> Result<()> {
    run(
        options.project_root,
        &options.file,
        &options.tag,
        options.num_tasks,
        options.no_expand,
        options.no_check_deps,
        options.append,
        options.no_guidance,
        &options.id_format,
        options.model.as_deref(),
        options.dry_run,
        options.verbose,
    )
    .await
}

/// Run the generate pipeline: parse PRD → expand tasks → validate dependencies
///
/// This is the internal implementation used by the CLI. For programmatic usage,
/// prefer the [`generate`] function with [`GenerateOptions`].
#[allow(clippy::too_many_arguments)]
pub async fn run(
    project_root: Option<PathBuf>,
    file: &Path,
    tag: &str,
    num_tasks: u32,
    no_expand: bool,
    no_check_deps: bool,
    append: bool,
    no_guidance: bool,
    id_format: &str,
    model: Option<&str>,
    dry_run: bool,
    verbose: bool,
) -> Result<()> {
    println!("{}", "━".repeat(50).blue());
    println!(
        "{} {}",
        "Generate Pipeline".blue().bold(),
        format!("(tag: {})", tag).cyan()
    );
    println!("{}", "━".repeat(50).blue());
    println!();

    if dry_run {
        println!("{} Dry run mode - no changes will be made", "ℹ".blue());
        println!();
    }

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Parse PRD into tasks
    // ═══════════════════════════════════════════════════════════════════════
    println!(
        "{} Parsing PRD into tasks...",
        "Phase 1:".yellow().bold()
    );

    if dry_run {
        println!(
            "  {} Would parse {} into tag '{}'",
            "→".cyan(),
            file.display(),
            tag
        );
        println!(
            "  {} Would create ~{} tasks (append: {})",
            "→".cyan(),
            num_tasks,
            append
        );
    } else {
        ai::parse_prd::run(
            project_root.clone(),
            file,
            tag,
            num_tasks,
            append,
            no_guidance,
            id_format,
            model,
        )
        .await?;
    }

    if verbose {
        println!("  {} Parse phase completed", "✓".green());
    }
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Expand complex tasks into subtasks
    // ═══════════════════════════════════════════════════════════════════════
    if no_expand {
        println!(
            "{} Skipping expansion {}",
            "Phase 2:".yellow().bold(),
            "(--no-expand)".dimmed()
        );
    } else {
        println!(
            "{} Expanding complex tasks into subtasks...",
            "Phase 2:".yellow().bold()
        );

        if dry_run {
            println!(
                "  {} Would expand tasks with complexity >= 5 in tag '{}'",
                "→".cyan(),
                tag
            );
        } else {
            ai::expand::run(
                project_root.clone(),
                None,      // task_id - expand all
                false,     // all_tags - only current tag
                Some(tag), // tag
                no_guidance,
                model,
            )
            .await?;
        }

        if verbose {
            println!("  {} Expand phase completed", "✓".green());
        }
    }
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Phase 3: Validate dependencies
    // ═══════════════════════════════════════════════════════════════════════
    if no_check_deps {
        println!(
            "{} Skipping dependency validation {}",
            "Phase 3:".yellow().bold(),
            "(--no-check-deps)".dimmed()
        );
    } else {
        println!(
            "{} Validating task dependencies...",
            "Phase 3:".yellow().bold()
        );

        if dry_run {
            println!(
                "  {} Would validate dependencies in tag '{}'",
                "→".cyan(),
                tag
            );
        } else {
            // Run check-deps without PRD validation (just structural checks)
            // Use a separate result to avoid early exit on dep issues
            let check_result = check_deps::run(
                project_root.clone(),
                Some(tag), // tag
                false,     // all_tags
                None,      // prd_file - no PRD validation in generate
                false,     // fix
                model,
            )
            .await;

            // Log but don't fail the pipeline on dep issues
            if let Err(e) = check_result {
                println!(
                    "  {} Dependency check encountered issues: {}",
                    "⚠".yellow(),
                    e
                );
                println!(
                    "  {} Run '{}' to see details",
                    "ℹ".blue(),
                    "scud check-deps".green()
                );
            }
        }

        if verbose {
            println!("  {} Check-deps phase completed", "✓".green());
        }
    }
    println!();

    // ═══════════════════════════════════════════════════════════════════════
    // Summary
    // ═══════════════════════════════════════════════════════════════════════
    println!("{}", "━".repeat(50).green());
    println!("{}", "✅ Generate pipeline complete!".green().bold());
    println!("{}", "━".repeat(50).green());
    println!();

    if dry_run {
        println!("{}", "Dry run - no changes were made.".yellow());
        println!("Run without --dry-run to execute the pipeline.");
    } else {
        println!("{}", "Next steps:".blue());
        println!("  1. Review tasks: scud list --tag {}", tag);
        println!("  2. View execution waves: scud waves --tag {}", tag);
        println!("  3. Start working: scud next --tag {}", tag);
    }
    println!();

    Ok(())
}
