use anyhow::Result;
use std::time::Duration;

use crate::commands::spawn::terminal::Harness;
use crate::storage::Storage;
use crate::SwarmMode;

use super::{
    events::EventWriter, execute_round, execute_round_extensions, execute_round_headless,
    execute_round_server, wait_for_round_completion, TaskInfo,
};

/// Runtime abstraction for swarm round execution.
///
/// This keeps the high-level swarm loop mode-agnostic while routing each
/// round through a specific backend adapter.
#[derive(Clone, Copy)]
pub(super) enum SwarmRuntime {
    /// Uses tmux and `scud attach`-style windows.
    Tmux,
    /// Uses extension subprocesses without tmux.
    Extensions,
    /// Uses OpenCode server orchestration.
    Server,
    /// Uses headless streaming runners.
    Headless,
    /// Continuous beads execution (orchestrated by `beads.rs`).
    Beads,
}

impl From<SwarmMode> for SwarmRuntime {
    fn from(value: SwarmMode) -> Self {
        match value {
            SwarmMode::Tmux => Self::Tmux,
            SwarmMode::Extensions => Self::Extensions,
            SwarmMode::Server => Self::Server,
            SwarmMode::Headless => Self::Headless,
            SwarmMode::Beads => Self::Beads,
        }
    }
}

impl SwarmRuntime {
    /// Human-facing label used for session metadata and output.
    pub(super) fn terminal_label(&self) -> &'static str {
        match self {
            Self::Tmux => "tmux",
            Self::Extensions => "extensions",
            Self::Server => "server",
            Self::Headless => "headless",
            Self::Beads => "beads",
        }
    }

    /// Human-facing mode label for startup output.
    pub(super) fn display_label(&self) -> &'static str {
        match self {
            Self::Tmux => "tmux (waves)",
            Self::Extensions => "extensions (waves)",
            Self::Server => "server (opencode)",
            Self::Headless => "headless (waves)",
            Self::Beads => "beads (continuous)",
        }
    }

    /// Whether this backend requires tmux availability.
    pub(super) fn needs_tmux(&self) -> bool {
        matches!(self, Self::Tmux | Self::Beads)
    }

    /// Whether this backend is tmux based and benefits from tmux recovery checks.
    pub(super) fn is_tmux(&self) -> bool {
        matches!(self, Self::Tmux)
    }

    /// Whether this backend is beads mode.
    pub(super) fn is_beads(&self) -> bool {
        matches!(self, Self::Beads)
    }

    /// Runtime-specific preflight checks.
    pub(super) fn ensure_requirements(&self) -> Result<()> {
        if self.needs_tmux() {
            crate::commands::spawn::terminal::check_tmux_available()?;
        }
        Ok(())
    }

    /// Run one round of tasks using this runtime.
    pub(super) async fn run_round(
        &self,
        storage: &Storage,
        tasks: &[TaskInfo<'_>],
        working_dir: &std::path::Path,
        session_name: &str,
        round_idx: usize,
        default_harness: Harness,
        stale_timeout: Option<Duration>,
        idle_timeout_minutes: u64,
        event_writer: Option<&EventWriter>,
    ) -> Result<super::session::RoundState> {
        let round_state = match self {
            Self::Tmux => {
                let state = execute_round(
                    storage,
                    tasks,
                    working_dir,
                    session_name,
                    round_idx,
                    default_harness,
                    event_writer,
                )?;
                wait_for_round_completion(
                    storage,
                    tasks,
                    session_name,
                    stale_timeout,
                    idle_timeout_minutes,
                    event_writer,
                )?;
                state
            }
            Self::Extensions => {
                execute_round_extensions(storage, tasks, working_dir, round_idx, default_harness)
                    .await?
            }
            Self::Server => execute_round_server(storage, tasks, working_dir, round_idx).await?,
            Self::Headless => {
                execute_round_headless(
                    storage,
                    tasks,
                    working_dir,
                    round_idx,
                    default_harness,
                    event_writer,
                )
                .await?
            }
            Self::Beads => {
                anyhow::bail!("Beads mode is handled outside round execution");
            }
        };

        Ok(round_state)
    }
}
