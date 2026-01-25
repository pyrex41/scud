//! OpenCode Server lifecycle management
//!
//! Manages the OpenCode server process lifecycle, including automatic startup,
//! health checking, and graceful shutdown.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

use super::client::OpenCodeClient;
use super::events::EventStream;

/// Default port for OpenCode server
pub const DEFAULT_PORT: u16 = 4096;

/// Server startup timeout
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

/// Health check interval during startup
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Configuration for OpenCode server
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Port to run on
    pub port: u16,
    /// Working directory for the server
    pub working_dir: Option<PathBuf>,
    /// Custom opencode binary path
    pub binary_path: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            working_dir: None,
            binary_path: None,
        }
    }
}

/// Manager for OpenCode server lifecycle
pub struct OpenCodeManager {
    config: ServerConfig,
    client: OpenCodeClient,
    server_process: Arc<RwLock<Option<Child>>>,
}

impl OpenCodeManager {
    /// Create a new manager with default config
    pub fn new() -> Self {
        Self::with_config(ServerConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: ServerConfig) -> Self {
        let client = OpenCodeClient::localhost(config.port);
        Self {
            config,
            client,
            server_process: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the HTTP client
    pub fn client(&self) -> &OpenCodeClient {
        &self.client
    }

    /// Check if server is running
    pub async fn is_running(&self) -> bool {
        self.client.health_check().await.unwrap_or(false)
    }

    /// Ensure server is running, starting it if needed
    pub async fn ensure_running(&self) -> Result<()> {
        if self.is_running().await {
            return Ok(());
        }

        self.start_server().await
    }

    /// Start the OpenCode server
    pub async fn start_server(&self) -> Result<()> {
        // Check if already running
        if self.is_running().await {
            return Ok(());
        }

        // Find opencode binary
        let binary = self.find_binary()?;

        // Build command
        let mut cmd = Command::new(&binary);
        cmd.arg("serve");
        cmd.arg("--port").arg(self.config.port.to_string());

        if let Some(ref dir) = self.config.working_dir {
            cmd.current_dir(dir);
        }

        // Suppress output (server runs in background)
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        // Spawn server process
        let child = cmd
            .spawn()
            .with_context(|| format!("Failed to start opencode server: {}", binary.display()))?;

        // Store process handle
        {
            let mut process = self.server_process.write().await;
            *process = Some(child);
        }

        // Wait for server to become ready
        self.wait_for_ready().await?;

        Ok(())
    }

    /// Wait for server to become ready
    async fn wait_for_ready(&self) -> Result<()> {
        let start = std::time::Instant::now();

        while start.elapsed() < STARTUP_TIMEOUT {
            if self.client.health_check().await.unwrap_or(false) {
                return Ok(());
            }
            tokio::time::sleep(HEALTH_CHECK_INTERVAL).await;
        }

        anyhow::bail!(
            "OpenCode server failed to start within {:?}",
            STARTUP_TIMEOUT
        );
    }

    /// Stop the server
    pub async fn stop_server(&self) -> Result<()> {
        let mut process = self.server_process.write().await;

        if let Some(mut child) = process.take() {
            // Try graceful shutdown first
            let _ = child.kill().await;
        }

        Ok(())
    }

    /// Find the opencode binary
    fn find_binary(&self) -> Result<PathBuf> {
        // Check custom path first
        if let Some(ref path) = self.config.binary_path {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Use find_harness_binary from terminal module
        use crate::commands::spawn::terminal::{find_harness_binary, Harness};

        find_harness_binary(Harness::OpenCode)
            .map(PathBuf::from)
            .context("Could not find opencode binary. Install with: npm install -g @anthropics/opencode")
    }

    /// Connect to the event stream
    pub async fn event_stream(&self) -> Result<EventStream> {
        self.ensure_running().await?;
        EventStream::connect(&self.client.event_stream_url()).await
    }

    /// Get the server port
    pub fn port(&self) -> u16 {
        self.config.port
    }

    /// Get the configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
}

impl Default for OpenCodeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for OpenCodeManager {
    fn drop(&mut self) {
        // Note: Can't do async cleanup in Drop
        // Server process will be orphaned but that's OK -
        // it can be reused by next SCUD invocation
    }
}

/// Global manager instance for sharing across swarm execution
static GLOBAL_MANAGER: std::sync::OnceLock<Arc<OpenCodeManager>> = std::sync::OnceLock::new();

/// Get or create the global manager instance
pub fn global_manager() -> Arc<OpenCodeManager> {
    GLOBAL_MANAGER
        .get_or_init(|| Arc::new(OpenCodeManager::new()))
        .clone()
}

/// Get or create manager with custom config (only works on first call)
pub fn init_global_manager(config: ServerConfig) -> Arc<OpenCodeManager> {
    GLOBAL_MANAGER
        .get_or_init(|| Arc::new(OpenCodeManager::with_config(config)))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.port, DEFAULT_PORT);
        assert!(config.working_dir.is_none());
        assert!(config.binary_path.is_none());
    }

    #[test]
    fn test_custom_config() {
        let config = ServerConfig {
            port: 8080,
            working_dir: Some(PathBuf::from("/tmp")),
            binary_path: Some(PathBuf::from("/usr/bin/opencode")),
        };
        assert_eq!(config.port, 8080);
        assert_eq!(config.working_dir, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn test_manager_creation() {
        let manager = OpenCodeManager::new();
        assert_eq!(manager.port(), DEFAULT_PORT);
    }

    #[test]
    fn test_manager_with_config() {
        let config = ServerConfig {
            port: 9000,
            ..Default::default()
        };
        let manager = OpenCodeManager::with_config(config);
        assert_eq!(manager.port(), 9000);
    }

    #[test]
    fn test_client_access() {
        let manager = OpenCodeManager::new();
        let client = manager.client();
        assert_eq!(client.base_url(), "http://127.0.0.1:4096");
    }
}
