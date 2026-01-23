//! Serve command - Run JSON RPC server for IPC with subagents
//!
//! This command starts a JSON RPC server that reads requests from stdin
//! and emits events/responses to stdout, enabling external orchestrators
//! to control SCUD agents programmatically.

use anyhow::Result;
use std::path::PathBuf;

use crate::commands::spawn::terminal::Harness;
use crate::rpc::{RpcServer, RpcServerConfig};

/// Main entry point for the serve command
pub async fn run(
    project_root: Option<PathBuf>,
    harness_arg: &str,
    model: Option<&str>,
) -> Result<()> {
    // Parse harness
    let harness = Harness::parse(harness_arg)?;

    // Get working directory
    let working_dir = project_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Create server config
    let config = RpcServerConfig {
        working_dir: working_dir.clone(),
        default_harness: harness,
        default_model: model.map(String::from),
        project_root,
    };

    // Create and run server
    let mut server = RpcServer::new(config);
    server.run().await
}
