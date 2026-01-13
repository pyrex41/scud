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

    /// Manage SCUD agents (Claude Code slash commands)
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
    /// Build and install SCUD binary (replaces npm install)
    Install,

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

        /// Output as JSON instead of human-readable format
        #[arg(long)]
        json: bool,

        /// Output raw SCG format (default: human-readable)
        #[arg(short = 'v', long)]
        verbose: bool,
    },

    /// Open interactive task viewer in browser
    View,

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

    /// Find next available task
    Next {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Output machine-readable JSON for orchestrators
        #[arg(long)]
        spawn: bool,

        /// Search across all phases for globally-correct next task
        #[arg(long)]
        all_tags: bool,
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
    #[command(alias = "parse-prd")]
    Parse {
        /// Path to PRD/phase markdown file
        file: PathBuf,

        /// Phase tag to create
        #[arg(short, long)]
        tag: String,

        /// Number of tasks to generate (default: 10)
        #[arg(short = 'n', long, default_value = "10")]
        num_tasks: u32,

        /// Append tasks to existing tag instead of replacing
        #[arg(long)]
        append: bool,

        /// Skip loading guidance from .scud/guidance/
        #[arg(long)]
        no_guidance: bool,

        /// Task ID format: sequential (default) or uuid
        #[arg(long, default_value = "sequential")]
        id_format: String,

        /// Model to use for task generation (overrides config)
        #[arg(long)]
        model: Option<String>,
    },

    /// Clear all tasks (with confirmation)
    Clean {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,

        /// Only clean a specific tag
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Analyze task complexity (AI-powered)
    AnalyzeComplexity {
        /// Specific task ID (analyzes all if not provided)
        #[arg(short = 'i', long)]
        task: Option<String>,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Model to use for complexity analysis (overrides config)
        #[arg(long)]
        model: Option<String>,
    },

    /// Expand complex task into subtasks (AI-powered)
    Expand {
        /// Specific task ID to expand (expands all in current tag if not provided)
        #[arg(short = 'i', long)]
        task: Option<String>,

        /// Expand all tasks across ALL tags (default: current tag only)
        #[arg(short, long)]
        all: bool,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Skip loading guidance from .scud/guidance/
        #[arg(long)]
        no_guidance: bool,

        /// Model to use for subtask generation (overrides config)
        #[arg(long)]
        model: Option<String>,
    },

    /// Re-analyze and suggest cross-tag dependencies (AI-powered)
    ReanalyzeDeps {
        /// Tag to analyze (default: all tags)
        #[arg(short, long)]
        tag: Option<String>,

        /// Analyze all tags (default if no tag specified)
        #[arg(long)]
        all_tags: bool,

        /// Automatically apply suggestions without prompting
        #[arg(long)]
        apply: bool,

        /// Show suggestions without applying
        #[arg(long)]
        dry_run: bool,

        /// Model to use for dependency analysis (overrides config)
        #[arg(long)]
        model: Option<String>,
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

    /// Show who is working on what
    WhoIs {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Get multiple ready tasks at once (for orchestrators)
    NextBatch {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Maximum number of tasks to return
        #[arg(short, long, default_value = "5")]
        limit: usize,

        /// Search across all phases for globally-correct tasks
        #[arg(long)]
        all_tags: bool,
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

    /// [EXPERIMENTAL] Diagnose stuck task states
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

    /// Check dependency validity and optionally validate against PRD
    CheckDeps {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Check across all phases
        #[arg(long)]
        all_tags: bool,

        /// Path to PRD file to validate task coverage (AI-powered)
        #[arg(long)]
        prd: Option<PathBuf>,

        /// Auto-fix PRD coverage issues (requires --prd)
        #[arg(long)]
        fix: bool,

        /// Model to use for PRD validation (overrides config)
        #[arg(long)]
        model: Option<String>,
    },

    /// Generate Mermaid diagram of task graph
    Mermaid {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Include all phases in the diagram
        #[arg(long)]
        all_tags: bool,
    },

    /// Write a summary log entry for a task
    Log {
        /// Task ID to log for
        task_id: String,

        /// Summary text (100-200 words recommended)
        summary: String,

        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,
    },

    /// Show log entries for a task
    LogShow {
        /// Task ID
        task_id: String,
    },

    /// Quick orientation for new session (show recent commits, active sessions, next task)
    Warmup,

    /// Create a git commit with task context
    Commit {
        /// Commit message (uses task title if not provided)
        #[arg(short, long)]
        message: Option<String>,

        /// Stage all changes before committing
        #[arg(short, long)]
        all: bool,
    },

    /// Spawn parallel Claude Code agents in terminal windows
    Spawn {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Maximum agents to spawn (default: 5)
        #[arg(short = 'n', long, default_value = "5")]
        limit: usize,

        /// Search across all phases for ready tasks
        #[arg(long)]
        all_tags: bool,

        /// Terminal: auto, tmux, kitty, wezterm, iterm2 (default: auto)
        #[arg(short = 'T', long, default_value = "auto")]
        terminal: String,

        /// Show plan without spawning
        #[arg(long)]
        dry_run: bool,

        /// tmux session name (default: scud-<tag>)
        #[arg(long)]
        session: Option<String>,

        /// Attach to tmux session after spawn
        #[arg(long)]
        attach: bool,

        /// Start TUI monitor after spawn (recommended)
        #[arg(short, long)]
        monitor: bool,

        /// Mark spawned tasks as in-progress
        #[arg(short, long)]
        claim: bool,
    },

    /// Monitor spawn session with interactive TUI
    Monitor {
        /// Session name to monitor (auto-detects if only one exists)
        #[arg(short, long)]
        session: Option<String>,
    },

    /// List spawn sessions
    Sessions {
        /// Show detailed info for each session
        #[arg(short, long)]
        verbose: bool,
    },

    /// Run swarm mode - wave-based parallel execution with backpressure
    Swarm {
        /// Phase tag (uses active phase if not provided)
        #[arg(short, long)]
        tag: Option<String>,

        /// Maximum tasks per round (default: 5)
        #[arg(short = 'n', long, default_value = "5")]
        round_size: usize,

        /// Execute across all phases
        #[arg(long)]
        all_tags: bool,

        /// Terminal: auto, tmux, kitty, wezterm, iterm2 (default: auto)
        #[arg(short = 'T', long, default_value = "auto")]
        terminal: String,

        /// Show execution plan without spawning
        #[arg(long)]
        dry_run: bool,

        /// Session name (default: swarm-<tag>)
        #[arg(long)]
        session: Option<String>,

        /// Skip research phase (use existing tasks as-is)
        #[arg(long)]
        no_research: bool,

        /// Skip backpressure validation after wave
        #[arg(long)]
        no_validate: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install => commands::install::run(),
        Commands::Init { provider } => commands::init::run(cli.project, provider),
        Commands::Tags { tag } => commands::tags::run(cli.project, tag.as_deref()),
        Commands::List {
            status,
            tag,
            json,
            verbose,
        } => commands::list::run(
            cli.project,
            status.as_deref(),
            tag.as_deref(),
            json,
            verbose,
        ),
        Commands::View => commands::view::run(cli.project),
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
            spawn,
            all_tags,
        } => commands::next::run(cli.project, tag.as_deref(), spawn, all_tags),
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
        Commands::Parse {
            file,
            tag,
            num_tasks,
            append,
            no_guidance,
            id_format,
            model,
        } => {
            commands::ai::parse_prd::run(
                cli.project,
                &file,
                &tag,
                num_tasks,
                append,
                no_guidance,
                &id_format,
                model.as_deref(),
            )
            .await
        }
        Commands::Clean { force, tag } => commands::clean::run(cli.project, force, tag.as_deref()),
        Commands::AnalyzeComplexity { task, tag, model } => {
            commands::ai::analyze_complexity::run(
                cli.project,
                task.as_deref(),
                tag.as_deref(),
                model.as_deref(),
            )
            .await
        }
        Commands::Expand {
            task,
            all,
            tag,
            no_guidance,
            model,
        } => {
            commands::ai::expand::run(
                cli.project,
                task.as_deref(),
                all,
                tag.as_deref(),
                no_guidance,
                model.as_deref(),
            )
            .await
        }
        Commands::ReanalyzeDeps {
            tag,
            all_tags,
            apply,
            dry_run,
            model,
        } => {
            commands::ai::reanalyze_deps::run(
                cli.project,
                tag.as_deref(),
                all_tags,
                apply,
                dry_run,
                model.as_deref(),
            )
            .await
        }
        Commands::Assign {
            task_id,
            assignee,
            tag,
        } => commands::assign::run(cli.project, &task_id, &assignee, tag.as_deref()),
        Commands::WhoIs { tag } => commands::whois::run(cli.project, tag.as_deref()),
        Commands::NextBatch {
            tag,
            limit,
            all_tags,
        } => commands::next_batch::run(cli.project, tag.as_deref(), limit, all_tags),
        Commands::Convert { from, to, backup } => {
            commands::convert::run(cli.project, &from, &to, backup)
        }
        Commands::Doctor {
            tag,
            stale_hours,
            fix,
        } => commands::doctor::run(cli.project, tag.as_deref(), stale_hours, fix),
        Commands::CheckDeps {
            tag,
            all_tags,
            prd,
            fix,
            model,
        } => {
            commands::check_deps::run(
                cli.project,
                tag.as_deref(),
                all_tags,
                prd.as_deref(),
                fix,
                model.as_deref(),
            )
            .await
        }
        Commands::Mermaid { tag, all_tags } => {
            commands::mermaid::run(cli.project, tag.as_deref(), all_tags)
        }
        Commands::Log {
            task_id,
            summary,
            tag,
        } => commands::log::run(cli.project, &task_id, &summary, tag.as_deref()),
        Commands::LogShow { task_id } => commands::log::show(cli.project, &task_id),
        Commands::Warmup => commands::warmup::run(cli.project),
        Commands::Commit { message, all } => {
            commands::commit::run(cli.project, message.as_deref(), all)
        }
        Commands::Spawn {
            tag,
            limit,
            all_tags,
            terminal,
            dry_run,
            session,
            attach,
            monitor,
            claim,
        } => commands::spawn::run(
            cli.project,
            tag.as_deref(),
            limit,
            all_tags,
            &terminal,
            dry_run,
            session,
            attach,
            monitor,
            claim,
        ),
        Commands::Monitor { session } => commands::spawn::run_monitor(cli.project, session),
        Commands::Sessions { verbose } => commands::spawn::run_sessions(cli.project, verbose),
        Commands::Swarm {
            tag,
            round_size,
            all_tags,
            terminal,
            dry_run,
            session,
            no_research,
            no_validate,
        } => commands::swarm::run(
            cli.project,
            tag.as_deref(),
            round_size,
            all_tags,
            &terminal,
            dry_run,
            session,
            no_research,
            no_validate,
        ),
    }
}
