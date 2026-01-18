# Swarm Monitor Integration Plan

## Overview

Integrate the spawn TUI monitor into swarm, then refactor to a unified session trait so both commands share the same monitoring infrastructure. This reduces confusion between overlapping spawn/swarm functionality.

## Current State Analysis

### The Problem
- **Spawn** has a nice three-panel TUI monitor (`--monitor` flag)
- **Swarm** has its own session system with no TUI visualization
- Users are confused about which command to use
- Code is duplicated between the two systems

### Current Architecture

| Aspect | Spawn | Swarm |
|--------|-------|-------|
| Session file | `.scud/spawn/{name}.json` | `.scud/swarm/{name}.json` |
| Data model | `SpawnSession` with flat `Vec<AgentState>` | `SwarmSession` with `Vec<WaveState>` containing `Vec<RoundState>` |
| TUI | Yes (`spawn/tui/`) | No |
| Wave computation | In TUI only (`app.rs:697`) | In main loop (`mod.rs:456`) |
| Agent tracking | `AgentStatus` enum | Task IDs in `RoundState.task_ids` |

### Key Discoveries
- Swarm already imports spawn's terminal and agent modules (`mod.rs:33-35`)
- SwarmSession tracks waves/rounds but NOT individual agent status
- The TUI's wave computation duplicates swarm's logic
- Both use the same tmux spawning infrastructure

## Desired End State

After implementation:
1. `scud swarm --monitor` launches a TUI that shows real-time wave/agent progress
2. A `MonitorableSession` trait abstracts session differences
3. Single TUI codebase works with either spawn or swarm sessions
4. Swarm console output remains available as default (no TUI)

### Verification
- `scud swarm --tag foo --monitor` shows three-panel TUI with waves, agents, output
- `scud swarm --tag foo` (no flag) works as before with console output
- `scud spawn --monitor` continues to work unchanged
- All existing tests pass

## What We're NOT Doing

- NOT deprecating spawn command (future consideration)
- NOT changing swarm's core wave/validation logic
- NOT adding Ralph mode to swarm (spawn-only feature)
- NOT merging the session storage formats (they serve different purposes)

## Implementation Approach

Two phases:
1. **Phase 1**: Quick integration - add `--monitor` to swarm with adapter
2. **Phase 2**: Clean architecture - extract `MonitorableSession` trait

---

## Phase 1: Add --monitor Flag to Swarm

### Overview
Add a `--monitor` flag to swarm that launches the existing TUI with an adapter that converts SwarmSession data to what the TUI expects.

### Changes Required

#### 1.1 Add CLI Flag

**File**: `scud-cli/src/main.rs`
**Changes**: Add `--monitor` flag to Swarm command

```rust
// Around line 625, in the Swarm command definition
Swarm {
    // ... existing fields ...

    /// Start TUI monitor for real-time visualization
    #[arg(short, long)]
    monitor: bool,
}
```

Update the match arm around line 953:
```rust
Commands::Swarm {
    tag,
    round_size,
    all_tags,
    harness,
    dry_run,
    session,
    no_research,
    no_validate,
    review,
    review_all,
    no_repair,
    max_repair_attempts,
    monitor,  // Add this
} => commands::swarm::run(
    cli.project,
    tag.as_deref(),
    round_size,
    all_tags,
    &harness,
    dry_run,
    session,
    no_research,
    no_validate,
    review,
    review_all,
    no_repair,
    max_repair_attempts,
    monitor,  // Add this
),
```

#### 1.2 Add SwarmSession to AgentState Conversion

**File**: `scud-cli/src/commands/swarm/session.rs`
**Changes**: Add method to convert SwarmSession to spawn's format

```rust
use crate::commands::spawn::monitor::{AgentState, AgentStatus, SpawnSession};

impl SwarmSession {
    /// Convert to SpawnSession format for TUI compatibility
    pub fn to_spawn_session(&self) -> SpawnSession {
        let mut agents = Vec::new();

        for wave in &self.waves {
            for round in &wave.rounds {
                for (idx, task_id) in round.task_ids.iter().enumerate() {
                    let tag = round.tags.get(idx).cloned().unwrap_or_default();
                    let failed = round.failures.contains(task_id);

                    agents.push(AgentState {
                        task_id: task_id.clone(),
                        task_title: task_id.clone(), // Will be enriched by TUI
                        window_name: format!("task-{}", task_id),
                        status: if failed {
                            AgentStatus::Failed
                        } else {
                            AgentStatus::Running
                        },
                        started_at: wave.started_at.clone(),
                        tag,
                    });
                }
            }
        }

        SpawnSession {
            session_name: self.session_name.clone(),
            tag: self.tag.clone(),
            terminal: self.terminal.clone(),
            created_at: self.created_at.clone(),
            working_dir: self.working_dir.clone(),
            agents,
        }
    }
}
```

#### 1.3 Add Monitor Mode to Swarm Run

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Add monitor parameter and launch TUI when enabled

Update function signature around line 47:
```rust
#[allow(clippy::too_many_arguments)]
pub fn run(
    project_root: Option<PathBuf>,
    tag: Option<&str>,
    round_size: usize,
    all_tags: bool,
    harness_arg: &str,
    dry_run: bool,
    session_name: Option<String>,
    no_research: bool,
    no_validate: bool,
    review: bool,
    review_all: bool,
    no_repair: bool,
    max_repair_attempts: usize,
    monitor: bool,  // Add this parameter
) -> Result<()> {
```

Add import at top:
```rust
use crate::commands::spawn::tui;
use crate::commands::spawn::monitor as spawn_monitor;
```

After session initialization (around line 191), add monitor check:
```rust
    // Initialize swarm session
    let mut swarm_session = SwarmSession::new(
        &session_name,
        &phase_tag,
        "tmux",
        &working_dir.to_string_lossy(),
        round_size,
    );

    // If monitor mode, convert to spawn session format and launch TUI
    if monitor && !dry_run {
        // Save initial session for TUI to find
        let spawn_session = swarm_session.to_spawn_session();
        spawn_monitor::save_session(project_root.as_ref(), &spawn_session)?;

        println!();
        println!("{}", "Starting monitor...".cyan());
        thread::sleep(Duration::from_secs(1));

        // Launch TUI in a separate thread, continue swarm in main thread
        let session_name_clone = session_name.clone();
        let project_root_clone = project_root.clone();
        std::thread::spawn(move || {
            let _ = tui::run(project_root_clone, &session_name_clone);
        });
    }
```

Update session save in main loop (around line 423) to also save spawn format:
```rust
        // Save session state
        swarm_session.waves.push(wave_state);
        session::save_session(project_root.as_ref(), &swarm_session)?;

        // Also save spawn-format session for TUI
        if monitor {
            let spawn_session = swarm_session.to_spawn_session();
            spawn_monitor::save_session(project_root.as_ref(), &spawn_session)?;
        }
```

#### 1.4 Make spawn/monitor Module Public

**File**: `scud-cli/src/commands/spawn/mod.rs`
**Changes**: Export monitor module publicly

```rust
// Change from private to public
pub mod monitor;
```

### Success Criteria

#### Automated Verification:
- [ ] Build passes: `cargo build`
- [ ] All tests pass: `cargo test`
- [ ] Clippy passes: `cargo clippy`
- [ ] Format check: `cargo fmt --check`

#### Manual Verification:
- [ ] `scud swarm --tag test --monitor` launches TUI alongside swarm execution
- [ ] TUI shows waves panel with tasks grouped by wave
- [ ] TUI shows agents panel with running/completed/failed agents
- [ ] TUI shows live terminal output from selected agent
- [ ] Console still shows swarm progress messages (TUI runs alongside)
- [ ] Ctrl+C in TUI doesn't kill swarm execution

**Implementation Note**: After completing Phase 1 and all automated verification passes, pause here for manual confirmation before proceeding to Phase 2.

---

## Phase 2: Unified MonitorableSession Trait

### Overview
Extract a trait that both SpawnSession and SwarmSession implement, allowing the TUI to work with either session type without conversion overhead.

### Changes Required

#### 2.1 Define MonitorableSession Trait

**File**: `scud-cli/src/commands/spawn/monitor.rs` (new section)
**Changes**: Add trait definition

```rust
/// Trait for sessions that can be displayed in the TUI monitor
pub trait MonitorableSession: Send + Sync {
    /// Get the session name
    fn session_name(&self) -> &str;

    /// Get the tag/phase being worked on
    fn tag(&self) -> &str;

    /// Get the working directory
    fn working_dir(&self) -> &str;

    /// Get all agents with their current status
    fn agents(&self) -> Vec<AgentView>;

    /// Get computed waves for display
    fn waves(&self) -> Vec<WaveView>;

    /// Get status counts for header display
    fn status_counts(&self) -> StatusCounts;

    /// Reload session data from disk
    fn refresh(&mut self) -> anyhow::Result<()>;
}

/// Read-only view of an agent for display
#[derive(Clone, Debug)]
pub struct AgentView {
    pub task_id: String,
    pub task_title: String,
    pub window_name: String,
    pub status: AgentStatus,
    pub tag: String,
}

/// Read-only view of a wave for display
#[derive(Clone, Debug)]
pub struct WaveView {
    pub wave_number: usize,
    pub tasks: Vec<WaveTaskView>,
}

#[derive(Clone, Debug)]
pub struct WaveTaskView {
    pub task_id: String,
    pub task_title: String,
    pub state: WaveTaskState,
    pub complexity: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WaveTaskState {
    Ready,
    Running,
    Done,
    Blocked,
    InProgress,
}

#[derive(Clone, Debug, Default)]
pub struct StatusCounts {
    pub starting: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
}
```

#### 2.2 Implement Trait for SpawnSession

**File**: `scud-cli/src/commands/spawn/monitor.rs`
**Changes**: Implement trait for existing SpawnSession

```rust
impl MonitorableSession for SpawnSession {
    fn session_name(&self) -> &str {
        &self.session_name
    }

    fn tag(&self) -> &str {
        &self.tag
    }

    fn working_dir(&self) -> &str {
        &self.working_dir
    }

    fn agents(&self) -> Vec<AgentView> {
        self.agents.iter().map(|a| AgentView {
            task_id: a.task_id.clone(),
            task_title: a.task_title.clone(),
            window_name: a.window_name.clone(),
            status: a.status.clone(),
            tag: a.tag.clone(),
        }).collect()
    }

    fn waves(&self) -> Vec<WaveView> {
        // SpawnSession doesn't track waves, return empty
        // The TUI computes waves dynamically from task storage
        Vec::new()
    }

    fn status_counts(&self) -> StatusCounts {
        StatusCounts {
            starting: self.count_by_status(AgentStatus::Starting),
            running: self.count_by_status(AgentStatus::Running),
            completed: self.count_by_status(AgentStatus::Completed),
            failed: self.count_by_status(AgentStatus::Failed),
        }
    }

    fn refresh(&mut self) -> anyhow::Result<()> {
        // SpawnSession refresh is handled by App::refresh()
        Ok(())
    }
}
```

#### 2.3 Implement Trait for SwarmSession

**File**: `scud-cli/src/commands/swarm/session.rs`
**Changes**: Implement trait for SwarmSession

```rust
use crate::commands::spawn::monitor::{
    AgentStatus, AgentView, MonitorableSession, StatusCounts, WaveTaskState, WaveTaskView, WaveView,
};

impl MonitorableSession for SwarmSession {
    fn session_name(&self) -> &str {
        &self.session_name
    }

    fn tag(&self) -> &str {
        &self.tag
    }

    fn working_dir(&self) -> &str {
        &self.working_dir
    }

    fn agents(&self) -> Vec<AgentView> {
        let mut agents = Vec::new();

        for wave in &self.waves {
            for round in &wave.rounds {
                for (idx, task_id) in round.task_ids.iter().enumerate() {
                    let tag = round.tags.get(idx).cloned().unwrap_or_default();
                    let failed = round.failures.contains(task_id);

                    // Determine status based on wave/task state
                    let status = if failed {
                        AgentStatus::Failed
                    } else if wave.validation.as_ref().map(|v| v.all_passed).unwrap_or(false) {
                        AgentStatus::Completed
                    } else {
                        AgentStatus::Running
                    };

                    agents.push(AgentView {
                        task_id: task_id.clone(),
                        task_title: task_id.clone(), // Enriched by TUI from storage
                        window_name: format!("task-{}", task_id),
                        status,
                        tag,
                    });
                }
            }
        }

        agents
    }

    fn waves(&self) -> Vec<WaveView> {
        self.waves.iter().map(|w| {
            let tasks: Vec<WaveTaskView> = w.rounds.iter()
                .flat_map(|r| {
                    r.task_ids.iter().map(|id| {
                        let failed = r.failures.contains(id);
                        let done = w.validation.as_ref()
                            .map(|v| v.all_passed)
                            .unwrap_or(false);

                        WaveTaskView {
                            task_id: id.clone(),
                            task_title: id.clone(),
                            state: if failed {
                                WaveTaskState::Blocked
                            } else if done {
                                WaveTaskState::Done
                            } else {
                                WaveTaskState::Running
                            },
                            complexity: None,
                        }
                    })
                })
                .collect();

            WaveView {
                wave_number: w.wave_number,
                tasks,
            }
        }).collect()
    }

    fn status_counts(&self) -> StatusCounts {
        let agents = self.agents();
        StatusCounts {
            starting: agents.iter().filter(|a| matches!(a.status, AgentStatus::Starting)).count(),
            running: agents.iter().filter(|a| matches!(a.status, AgentStatus::Running)).count(),
            completed: agents.iter().filter(|a| matches!(a.status, AgentStatus::Completed)).count(),
            failed: agents.iter().filter(|a| matches!(a.status, AgentStatus::Failed)).count(),
        }
    }

    fn refresh(&mut self) -> anyhow::Result<()> {
        // Reload from disk
        if let Ok(loaded) = load_session(None, &self.tag) {
            *self = loaded;
        }
        Ok(())
    }
}
```

#### 2.4 Update TUI App to Use Trait

**File**: `scud-cli/src/commands/spawn/tui/app.rs`
**Changes**: Make App generic over MonitorableSession

This is a larger refactor. Key changes:

1. Change `session: Option<SpawnSession>` to use trait object:
```rust
pub struct App {
    // Change from:
    // pub session: Option<SpawnSession>,
    // To:
    pub session: Option<Box<dyn MonitorableSession>>,
    // ... rest of fields
}
```

2. Update `App::new()` to accept session type:
```rust
pub fn new_with_session<S: MonitorableSession + 'static>(
    project_root: Option<PathBuf>,
    session: S,
) -> Self {
    // ...
    App {
        session: Some(Box::new(session)),
        // ...
    }
}
```

3. Update methods that access session to use trait methods instead of direct field access.

#### 2.5 Update TUI Entry Point

**File**: `scud-cli/src/commands/spawn/tui/mod.rs`
**Changes**: Add entry point that accepts trait object

```rust
/// Run TUI with any MonitorableSession implementation
pub fn run_with_session<S: MonitorableSession + 'static>(
    project_root: Option<PathBuf>,
    session: S,
) -> Result<()> {
    // ... terminal setup ...

    let mut app = App::new_with_session(project_root, session);
    run_app(&mut terminal, &mut app)?;

    // ... terminal cleanup ...
}
```

#### 2.6 Update Swarm to Use New Entry Point

**File**: `scud-cli/src/commands/swarm/mod.rs`
**Changes**: Use trait-based TUI entry

```rust
// Replace the thread::spawn approach from Phase 1 with:
if monitor && !dry_run {
    // Clone session for TUI thread
    let swarm_session_for_tui = swarm_session.clone();
    let project_root_clone = project_root.clone();

    std::thread::spawn(move || {
        let _ = tui::run_with_session(project_root_clone, swarm_session_for_tui);
    });
}
```

### Success Criteria

#### Automated Verification:
- [ ] Build passes: `cargo build`
- [ ] All tests pass: `cargo test`
- [ ] Clippy passes: `cargo clippy`
- [ ] Format check: `cargo fmt --check`

#### Manual Verification:
- [ ] `scud spawn --monitor` still works (backward compatible)
- [ ] `scud swarm --tag test --monitor` works with new trait system
- [ ] TUI displays swarm waves from SwarmSession directly (no conversion)
- [ ] Status counts update correctly as agents complete
- [ ] Agent selection and output viewing works for swarm sessions
- [ ] No performance regression with trait objects

**Implementation Note**: After completing Phase 2 and all verification passes, the integration is complete.

---

## Testing Strategy

### Unit Tests

**New tests in `spawn/monitor.rs`:**
```rust
#[cfg(test)]
mod trait_tests {
    use super::*;

    #[test]
    fn test_spawn_session_implements_trait() {
        let session = SpawnSession::new("test", "tag", "tmux", "/tmp");
        let _: &dyn MonitorableSession = &session;
    }

    #[test]
    fn test_agents_view_conversion() {
        let mut session = SpawnSession::new("test", "tag", "tmux", "/tmp");
        session.add_agent("task-1", "Title", "tag");

        let agents = session.agents();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].task_id, "task-1");
    }
}
```

**New tests in `swarm/session.rs`:**
```rust
#[cfg(test)]
mod trait_tests {
    use super::*;
    use crate::commands::spawn::monitor::MonitorableSession;

    #[test]
    fn test_swarm_session_implements_trait() {
        let session = SwarmSession::new("test", "tag", "tmux", "/tmp", 3);
        let _: &dyn MonitorableSession = &session;
    }

    #[test]
    fn test_waves_view() {
        let mut session = SwarmSession::new("test", "tag", "tmux", "/tmp", 3);
        let mut wave = WaveState::new(1);
        wave.rounds.push(RoundState {
            round_number: 0,
            task_ids: vec!["task-1".to_string()],
            tags: vec!["tag".to_string()],
            failures: vec![],
        });
        session.waves.push(wave);

        let waves = session.waves();
        assert_eq!(waves.len(), 1);
        assert_eq!(waves[0].tasks.len(), 1);
    }
}
```

### Integration Tests

Add to existing E2E tests:
```rust
#[test]
fn test_swarm_monitor_flag_accepted() {
    // Verify CLI accepts --monitor flag
    let output = Command::new("cargo")
        .args(["run", "--", "swarm", "--help"])
        .output()
        .expect("Failed to run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--monitor"));
}
```

### Manual Testing Steps

1. Create test tasks: `scud add "Test task 1" && scud add "Test task 2"`
2. Run swarm with monitor: `scud swarm --monitor --dry-run` (verify flag works)
3. Run swarm for real: `scud swarm --monitor --no-validate`
4. Verify TUI shows waves and agents
5. Select different agents, verify output panel updates
6. Wait for agents to complete, verify status changes

## Performance Considerations

- Trait object dispatch adds minimal overhead (~1ns per call)
- Session conversion in Phase 1 is O(n) where n = number of agents
- Phase 2 eliminates conversion overhead entirely
- TUI refresh rate (2s for status, 500ms for output) is unchanged

## Migration Notes

- No breaking changes to existing spawn usage
- No breaking changes to swarm without `--monitor` flag
- Session files remain in separate directories (`.scud/spawn/` vs `.scud/swarm/`)

## References

- Research document: `thoughts/shared/research/2026-01-18-monitor-feature-status.md`
- Spawn TUI entry: `scud-cli/src/commands/spawn/tui/mod.rs:28`
- Swarm main loop: `scud-cli/src/commands/swarm/mod.rs:195-426`
- SpawnSession: `scud-cli/src/commands/spawn/monitor.rs:33-42`
- SwarmSession: `scud-cli/src/commands/swarm/session.rs:208-277`
