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

    /// Manage SCUD workflow agents (Claude Code slash commands)
    Agents {
        #[command(subcommand)]
        command: AgentsCommands,
    },
}

#[derive(Subcommand)]
enum AgentsCommands {
    /// List installed SCUD agents
    List,

    /// Add SCUD agent(s) to the project
    Add {
        /// Agent name (pm, sm, architect, dev, retrospective, status) or use --all
        name: Option<String>,

        /// Add all SCUD agents
        #[arg(long)]
        all: bool,
    },

    /// Remove SCUD agent(s) from the project
    Remove {
        /// Agent name (pm, sm, architect, dev, retrospective, status) or use --all
        name: Option<String>,

        /// Remove all SCUD agents
        #[arg(long)]
        all: bool,
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

    /// List phase tags or set active tag
    Tags {
        /// Tag to set as active (lists tags if not provided)
        tag: Option<String>,
    },

    /// List tasks in active phase
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Show detailed task information
    Show {
        /// Task ID
        task_id: String,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Update task status
    SetStatus {
        /// Task ID
        task_id: String,
        /// New status
        status: String,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Find next available task (EXPERIMENTAL: use --claim for dynamic-wave mode)
    Next {
        /// Phase tag (uses active phase if not provided)
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

    /// Show phase statistics
    Stats {
        /// Phase tag (uses active phase if not provided)
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
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Maximum parallel tasks per round (default: 5, min: 1)
        #[arg(short = 'n', long, default_value = "5")]
        max_parallel: usize,

        /// Plan across all phases
        #[arg(long)]
        all_tags: bool,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Parse PRD/phase markdown into tasks (AI-powered)
    ParsePrd {
        /// Path to PRD/phase markdown file
        file: PathBuf,

        /// Phase tag to create
        #[arg(short, long)]
        tag: String,
    },

    /// Analyze task complexity (AI-powered)
    AnalyzeComplexity {
        /// Specific task ID (analyzes all if not provided)
        #[arg(short = 'i', long)]
        task: Option<String>,

        /// Phase tag (uses active phase if not provided)
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

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    // Task Assignment commands
    /// Assign task to a developer
    Assign {
        /// Task ID
        task_id: String,

        /// Assignee name
        assignee: String,

        /// Phase tag (uses active phase if not provided)
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

        /// Phase tag (uses active phase if not provided)
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

        /// Phase tag (uses active phase if not provided)
        #[arg(short = 'e', long)]
        tag: Option<String>,
    },

    /// Show who is working on what
    WhoIs {
        /// Phase tag (uses active phase if not provided)
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
        /// Phase tag (checks all phases if not provided)
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
            ConfigCommands::Agents { command } => match command {
                AgentsCommands::List => commands::config::agents_list(cli.project),
                AgentsCommands::Add { name, all } => {
                    commands::config::agents_add(cli.project, name, all)
                }
                AgentsCommands::Remove { name, all } => {
                    commands::config::agents_remove(cli.project, name, all)
                }
            },
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
