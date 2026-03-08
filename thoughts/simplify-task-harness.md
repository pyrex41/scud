# Simplify Task Harness — Plan

## Current State

The harness consists of ~4,500 lines across these key files:

| File | Lines | Role |
|------|-------|------|
| `swarm/mod.rs` | 2,720 | Main orchestrator — wave loop, 5 execution modes, repair, review |
| `spawn/mod.rs` | 870 | One-shot parallel spawn (tmux + headless) |
| `spawn/terminal.rs` | 860 | Tmux window management, harness binary resolution |
| `spawn/headless/runner.rs` | 1,905 | Headless streaming infrastructure |
| `spawn/agent.rs` | 769 | Agent config resolution, prompt generation |
| `swarm/session.rs` | 1,024 | Session state, wave execution (extensions mode) |
| `swarm/events.rs` | 966 | SQLite event logging + ZMQ publishing |
| `swarm/beads.rs` | 722 | Beads continuous-polling mode |
| `swarm/transcript.rs` | 873 | Transcript watching |

## Key Problems (Complexity Sources)

### 1. Five execution modes doing ~the same thing differently (~1,200 lines of near-duplication)

`swarm/mod.rs` has five separate `execute_round_*` functions:
- `execute_round` (Tmux) — spawns in tmux, polls with `wait_for_round_completion`
- `execute_round_extensions` — uses async subprocess runner
- `execute_round_server` — uses OpenCode Server orchestrator
- `execute_round_headless` — uses headless streaming
- `beads::run_beads_loop` — continuous polling variant

Each one: marks tasks in-progress → spawns agents → waits → collects results. The spawn and wait logic is copy-pasted with slight variations.

### 2. `swarm::run()` is a 900-line monolith with 20+ parameters

The signature alone (`#[allow(clippy::too_many_arguments)]`) is a code smell. It handles:
- Session locking
- Worktree setup
- Transcript watcher launch
- Heartbeat thread
- Orphan task detection (interactive!)
- Event writer + ZMQ publisher
- Control command pause/stop polling
- Wave loop (research → build → validate → review)
- Repair loop
- Salvo sync
- Final summary

### 3. `terminal.rs` has 4 spawn variants for normal + 3 ralph variants

- `spawn_terminal` → calls `spawn_terminal_with_harness_and_model`
- `spawn_terminal_with_harness` → calls `spawn_terminal_with_harness_and_model`
- `spawn_terminal_with_harness_and_model` → calls `spawn_tmux`
- `spawn_terminal_with_task_list` → calls `spawn_tmux`
- `spawn_terminal_ralph` → calls `spawn_terminal_ralph_with_harness`
- `spawn_terminal_ralph_with_harness` → calls `spawn_tmux_ralph`
- `spawn_in_tmux` — standalone simpler version

That's 7 public functions for 2 actual operations (spawn, spawn-with-loop), where the only differences are optional params.

### 4. Marker-file-based IPC for review and repair completion

Review and repair wait for completion by polling for magic files on disk (`.scud/review-complete-N`, `.scud/batch-repair-complete`). This is fragile and hard to debug.

### 5. Duplicated task-readiness logic

`spawn/mod.rs::is_task_ready()` and `swarm/mod.rs::is_task_actionable()` are near-identical.

---

## Proposed Simplifications

### Phase 1: Consolidate terminal spawn functions (terminal.rs)

**Before**: 7 public functions
**After**: 2 public functions

```rust
/// Single spawn entry point
pub fn spawn_tmux_agent(config: &SpawnConfig) -> Result<String> { ... }

/// Ralph loop variant
pub fn spawn_tmux_ralph(config: &SpawnConfig, completion_promise: &str) -> Result<()> { ... }

pub struct SpawnConfig<'a> {
    pub task_id: &'a str,
    pub prompt: &'a str,
    pub working_dir: &'a Path,
    pub session_name: &'a str,
    pub harness: Harness,
    pub model: Option<&'a str>,
    pub task_list_id: Option<&'a str>,
}
```

### Phase 2: Unify execution modes behind a trait

```rust
#[async_trait]
trait RoundExecutor {
    async fn execute(&self, tasks: &[TaskInfo], ctx: &RoundContext) -> Result<RoundState>;
}
```

Implementations: `TmuxExecutor`, `HeadlessExecutor`, `ExtensionsExecutor`, `ServerExecutor`

This collapses 4 separate `execute_round_*` functions into a single dispatch with shared pre/post logic (mark in-progress, emit events, mark results).

### Phase 3: Extract the wave loop body from `swarm::run()`

Break the 900-line function into:
1. `SwarmRunner::new(config)` — setup (locking, worktree, hooks, event writer)
2. `SwarmRunner::run_wave_loop()` — the main loop
3. `SwarmRunner::finalize()` — summary, sync, cleanup

Group the 20+ args into a `SwarmConfig` struct.

### Phase 4: Deduplicate task-readiness logic

Extract shared `is_task_ready()` into `models/task.rs` or a shared helper, used by both spawn and swarm.

### Phase 5: Replace marker-file IPC with channel-based completion

For repair and review, instead of polling the filesystem:
- Headless mode already has channels — use the same pattern
- Tmux mode: poll task status (which already works) instead of custom marker files

---

## What NOT to change

- **The wave/round/backpressure model** — this is the core value prop, it's sound
- **Beads mode** — it's already well-isolated in its own file
- **Agent definition TOML format** — stable, simple
- **Storage layer** — not in scope
- **Headless streaming infrastructure** — already well-structured

## Priority Order

1. **Phase 1** (terminal.rs consolidation) — highest bang-for-buck, zero risk
2. **Phase 4** (dedup task readiness) — trivial, prevents future divergence
3. **Phase 3** (extract SwarmConfig + SwarmRunner) — biggest impact on readability
4. **Phase 2** (RoundExecutor trait) — structural improvement, moderate effort
5. **Phase 5** (marker file removal) — nice-to-have, lower priority
