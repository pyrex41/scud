use anyhow::Result;
use clap::{Parser, Subcommand};
use scud::commands;
use std::path::PathBuf;

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show,

    /// Set LLM provider
    SetProvider {
        /// Provider name (xai, anthropic, openai, openrouter)
        provider: String,

        /// Optional model name (defaults to provider's default)
        #[arg(short, long)]
        model: Option<String>,
    },

    /// Set research-specific model (optional, overrides main model for research command)
    SetResearchModel {
        /// Model name (leave empty to clear and use main model)
        model: Option<String>,
    },
}

#[derive(Parser)]
#[command(name = "scud")]
#[command(about = "Fast, simple task master for AI-driven development", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Project root directory
    #[arg(short, long, global = true)]
    project: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize SCUD in current directory
    Init {
        /// LLM provider to use (xai, anthropic, openai, openrouter)
        #[arg(long)]
        provider: Option<String>,
    },

    /// List epic tags or set active tag
    Tags {
        /// Tag to set as active (lists tags if not provided)
        tag: Option<String>,
    },

    /// List tasks in active epic
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Show detailed task information
    Show {
        /// Task ID
        task_id: String,

        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Update task status
    SetStatus {
        /// Task ID
        task_id: String,
        /// New status
        status: String,

        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Find next available task (EXPERIMENTAL: use --claim for dynamic-wave mode)
    Next {
        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// [EXPERIMENTAL] Auto-claim the task for the specified agent
        #[arg(long, requires = "name")]
        claim: bool,

        /// Agent/developer name (required with --claim)
        #[arg(short, long)]
        name: Option<String>,

        /// [EXPERIMENTAL] Release the currently claimed task for this agent
        #[arg(long, conflicts_with = "claim", requires = "name")]
        release: bool,
    },

    /// Show epic statistics
    Stats {
        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Migrate task data to new format (namespaced IDs, parent-child relationships)
    Migrate {
        /// Show what would change without making changes
        #[arg(long)]
        dry_run: bool,
    },

    /// Plan parallel execution waves based on task dependencies
    Waves {
        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Maximum parallel tasks per round (default: 5, min: 1)
        #[arg(short = 'n', long, default_value = "5")]
        max_parallel: usize,

        /// Plan across all epics
        #[arg(long)]
        all_tags: bool,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Parse PRD/epic markdown into tasks (AI-powered)
    ParsePrd {
        /// Path to PRD/epic markdown file
        file: PathBuf,

        /// Epic tag to create
        #[arg(short, long)]
        tag: String,
    },

    /// Analyze task complexity (AI-powered)
    AnalyzeComplexity {
        /// Specific task ID (analyzes all if not provided)
        #[arg(short = 'i', long)]
        task: Option<String>,

        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Expand complex task into subtasks (AI-powered)
    Expand {
        /// Task ID to expand
        task_id: Option<String>,

        /// Expand all tasks with complexity > 13
        #[arg(short, long)]
        all: bool,

        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Research a topic using web search (AI-powered)
    Research {
        /// Research query
        query: String,
    },

    // Task Assignment commands
    /// Assign task to a developer
    Assign {
        /// Task ID
        task_id: String,

        /// Assignee name
        assignee: String,

        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Claim a task for yourself
    Claim {
        /// Task ID
        task_id: String,

        /// Your name/identifier
        #[arg(short, long)]
        name: String,

        /// Epic tag (uses active epic if not provided)
        #[arg(short = 'e', long)]
        tag: Option<String>,
    },

    /// Release task assignment/lock
    Release {
        /// Task ID
        task_id: String,

        /// Force release even if locked by someone else
        #[arg(short, long)]
        force: bool,

        /// Epic tag (uses active epic if not provided)
        #[arg(short = 'e', long)]
        tag: Option<String>,
    },

    /// Show who is working on what
    WhoIs {
        /// Epic tag (uses active epic if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Convert task storage format between JSON and SCG
    Convert {
        /// Source format (json, scg)
        #[arg(long)]
        from: String,

        /// Target format (json, scg)
        #[arg(long)]
        to: String,

        /// Create backup of source file (default: true)
        #[arg(long, default_value = "true")]
        backup: bool,
    },

    /// [EXPERIMENTAL] Diagnose stuck workflow states
    Doctor {
        /// Epic tag (checks all epics if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Stale lock threshold in hours (default: 24)
        #[arg(long, default_value = "24")]
        stale_hours: f64,

        /// Attempt auto-fix for recoverable issues
        #[arg(long)]
        fix: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { provider } => commands::init::run(cli.project, provider),
        Commands::Tags { tag } => commands::tags::run(cli.project, tag.as_deref()),
        Commands::List { status, tag } => {
            commands::list::run(cli.project, status.as_deref(), tag.as_deref())
        }
        Commands::Show { task_id, tag } => {
            commands::show::run(cli.project, &task_id, tag.as_deref())
        }
        Commands::SetStatus {
            task_id,
            status,
            tag,
        } => commands::set_status::run(cli.project, &task_id, &status, tag.as_deref()),
        Commands::Next {
            tag,
            claim,
            name,
            release,
        } => commands::next::run(cli.project, tag.as_deref(), claim, name.as_deref(), release),
        Commands::Stats { tag } => commands::stats::run(cli.project, tag.as_deref()),
        Commands::Migrate { dry_run } => commands::migrate::run(cli.project, dry_run),
        Commands::Waves {
            tag,
            max_parallel,
            all_tags,
        } => commands::waves::run(cli.project, tag.as_deref(), max_parallel, all_tags),
        Commands::Config { command } => match command {
            ConfigCommands::Show => commands::config::show(cli.project),
            ConfigCommands::SetProvider { provider, model } => {
                commands::config::set_provider(cli.project, &provider, model)
            }
            ConfigCommands::SetResearchModel { model } => {
                commands::config::set_research_model(cli.project, model)
            }
        },
        Commands::ParsePrd { file, tag } => {
            commands::ai::parse_prd::run(cli.project, &file, &tag).await
        }
        Commands::AnalyzeComplexity { task, tag } => {
            commands::ai::analyze_complexity::run(cli.project, task.as_deref(), tag.as_deref())
                .await
        }
        Commands::Expand { task_id, all, tag } => {
            commands::ai::expand::run(cli.project, task_id.as_deref(), all, tag.as_deref()).await
        }
        Commands::Research { query } => commands::ai::research::run(cli.project, &query).await,
        Commands::Assign {
            task_id,
            assignee,
            tag,
        } => commands::assign::run(cli.project, &task_id, &assignee, tag.as_deref()),
        Commands::Claim { task_id, name, tag } => {
            commands::claim::run(cli.project, &task_id, &name, tag.as_deref())
        }
        Commands::Release {
            task_id,
            force,
            tag,
        } => commands::release::run(cli.project, &task_id, force, tag.as_deref()),
        Commands::WhoIs { tag } => commands::whois::run(cli.project, tag.as_deref()),
        Commands::Convert { from, to, backup } => {
            commands::convert::run(cli.project, &from, &to, backup)
        }
        Commands::Doctor {
            tag,
            stale_hours,
            fix,
        } => commands::doctor::run(cli.project, tag.as_deref(), stale_hours, fix),
    }
}
