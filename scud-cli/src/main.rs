mod commands;
mod llm;
mod models;
mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    Init,

    /// List all epic tags
    Tags,

    /// Set active epic tag
    UseTag {
        /// Epic tag to activate
        tag: String,
    },

    /// List tasks in active epic
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },

    /// Show detailed task information
    Show {
        /// Task ID
        task_id: String,
    },

    /// Update task status
    SetStatus {
        /// Task ID
        task_id: String,
        /// New status
        status: String,
    },

    /// Find next available task
    Next,

    /// Show epic statistics
    Stats,

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
        #[arg(short, long)]
        task: Option<String>,
    },

    /// Expand complex task into subtasks (AI-powered)
    Expand {
        /// Task ID to expand
        task_id: Option<String>,

        /// Expand all tasks with complexity > 13
        #[arg(short, long)]
        all: bool,
    },

    /// Research a topic using web search (AI-powered)
    Research {
        /// Research query
        query: String,
    },

    // Epic Group commands
    /// Create a new epic group
    CreateGroup {
        /// Group name
        name: String,

        /// Comma-separated list of epic tags
        #[arg(short, long)]
        epics: String,

        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// List all epic groups
    ListGroups,

    /// Show group status and aggregated stats
    GroupStatus {
        /// Group ID
        group_id: String,
    },

    /// Add epic to a group
    AddToGroup {
        /// Group ID
        group_id: String,

        /// Epic tag to add
        epic_tag: String,
    },

    // Task Assignment commands
    /// Assign task to a developer
    Assign {
        /// Task ID
        task_id: String,

        /// Assignee name
        assignee: String,
    },

    /// Claim a task for yourself
    Claim {
        /// Task ID
        task_id: String,

        /// Your name/identifier
        #[arg(short, long)]
        name: String,
    },

    /// Release task assignment/lock
    Release {
        /// Task ID
        task_id: String,

        /// Force release even if locked by someone else
        #[arg(short, long)]
        force: bool,
    },

    /// Show who is working on what
    WhoIs,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => commands::init::run(cli.project),
        Commands::Tags => commands::tags::run(cli.project),
        Commands::UseTag { tag } => commands::use_tag::run(cli.project, &tag),
        Commands::List { status } => commands::list::run(cli.project, status.as_deref()),
        Commands::Show { task_id } => commands::show::run(cli.project, &task_id),
        Commands::SetStatus { task_id, status } => {
            commands::set_status::run(cli.project, &task_id, &status)
        }
        Commands::Next => commands::next::run(cli.project),
        Commands::Stats => commands::stats::run(cli.project),
        Commands::ParsePrd { file, tag } => {
            commands::ai::parse_prd::run(cli.project, &file, &tag).await
        }
        Commands::AnalyzeComplexity { task } => {
            commands::ai::analyze_complexity::run(cli.project, task.as_deref()).await
        }
        Commands::Expand { task_id, all } => {
            commands::ai::expand::run(cli.project, task_id.as_deref(), all).await
        }
        Commands::Research { query } => commands::ai::research::run(cli.project, &query).await,
        Commands::CreateGroup {
            name,
            epics,
            description,
        } => commands::create_group::run(cli.project, &name, &epics, description.as_deref()),
        Commands::ListGroups => commands::list_groups::run(cli.project),
        Commands::GroupStatus { group_id } => commands::group_status::run(cli.project, &group_id),
        Commands::AddToGroup {
            group_id,
            epic_tag,
        } => commands::add_to_group::run(cli.project, &group_id, &epic_tag),
        Commands::Assign {
            task_id,
            assignee,
        } => commands::assign::run(cli.project, &task_id, &assignee),
        Commands::Claim { task_id, name } => commands::claim::run(cli.project, &task_id, &name),
        Commands::Release { task_id, force } => {
            commands::release::run(cli.project, &task_id, force)
        }
        Commands::WhoIs => commands::whois::run(cli.project),
    }
}
