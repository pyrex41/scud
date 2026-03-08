pub mod ai;
pub mod check_deps;
pub mod clean;
pub mod commit;
pub mod config;
pub mod convert;
pub mod doctor;
pub mod generate;
pub mod helpers;
pub mod init;
pub mod install;
pub mod list;
pub mod log;
pub mod mermaid;
pub mod migrate;
pub mod next;
pub mod next_batch;
pub mod set_status;
pub mod show;
pub mod stats;
pub mod tags;
pub mod view;
pub mod warmup;
pub mod watch;
pub mod waves;

// Task assignment commands
pub mod assign;
pub mod whois;

// Parallel execution
pub mod spawn;

// Single agent execution
pub mod run;

// Headless session resumption
pub mod attach;

// Task restart (reset + spawn)
pub mod restart;

// Swarm mode (wave-based parallel execution with backpressure)
pub mod swarm;

// Salvo worktree management for parallel tag execution
pub mod salvo;

// Ralph mode (sequential iteration with fresh context)
pub mod ralph;

// Test and fix loop
pub mod task_selection;
pub mod test;

// JSON RPC IPC server for subagents
pub mod serve;

// Claude Tasks sync (internal)
pub mod sync_from_claude;

// Attractor pipeline commands
pub mod attractor;

// Direct API agent execution
#[cfg(feature = "direct-api")]
pub mod agent_exec;

// Heavy multi-agent reasoning mode
#[cfg(feature = "direct-api")]
pub mod heavy;
