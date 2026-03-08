pub mod check;
pub mod explain;
pub mod gate;
pub mod log;
pub mod manage;
pub mod record;
pub mod release;
pub mod status;
pub mod summary;
pub mod template;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum WeaveCommands {
    /// Evaluate whether an event would be allowed
    Check {
        /// Event as JSON string
        event_json: String,
    },
    /// Record that an event occurred (updates locks and event log)
    Record {
        /// Event as JSON string
        event_json: String,
    },
    /// Release a specific mutex lock
    Release {
        /// Lock key to release
        lock_key: String,
        /// Agent releasing the lock (optional)
        agent: Option<String>,
    },
    /// Release all locks held by an agent
    ReleaseAll {
        /// Agent whose locks to release
        #[arg(long)]
        agent: String,
        /// Only release locks for this task
        #[arg(long)]
        task: Option<String>,
    },
    /// Initialize @weave section in active phase SCG
    Init,
    /// Add a b-thread to the @weave section
    Add {
        /// Thread ID (e.g. "w:1")
        id: String,
        /// Thread name
        name: String,
        /// Rule type (Mutex, Require, BlockAlways, BlockUntil, RateLimit, Timeout, Partition)
        rule_type: String,
        /// Rule spec as key=value pairs
        #[arg(trailing_var_arg = true)]
        rule_spec: Vec<String>,
    },
    /// Enable a b-thread
    Enable {
        /// Thread ID
        id: String,
    },
    /// Disable a b-thread
    Disable {
        /// Thread ID
        id: String,
    },
    /// Remove a b-thread
    Remove {
        /// Thread ID
        id: String,
    },
    /// List all b-threads
    List,
    /// Show active locks, thread summary, and coordinator state
    Status,
    /// Show recent events from the event log
    Log {
        /// Number of recent events to show
        #[arg(long, default_value = "20")]
        tail: usize,
    },
    /// Explain why an event would be blocked (verbose output)
    Explain {
        /// Event as JSON string
        event_json: String,
    },
    /// Show weave summary for orientation
    Summary,
    /// Gate command for PreToolUse hook integration
    Gate {
        /// Tool name (e.g. Write, Bash, Edit)
        #[arg(long)]
        tool: String,
        /// Tool input as JSON string
        #[arg(long)]
        input: String,
    },
    /// Manage built-in b-thread templates
    Template {
        #[command(subcommand)]
        command: TemplateCommands,
    },
}

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// List available templates
    List,
    /// Apply a template (adds disabled b-thread to @weave)
    Apply {
        /// Template name
        name: String,
    },
}
