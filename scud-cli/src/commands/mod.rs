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
pub mod waves;

// Task assignment commands
pub mod assign;
pub mod whois;

// Parallel execution
pub mod spawn;

// Single agent execution
pub mod run;

// Swarm mode (wave-based parallel execution with backpressure)
pub mod swarm;
