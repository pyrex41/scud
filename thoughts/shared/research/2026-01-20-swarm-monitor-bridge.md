## Swarm Monitor Bridge Implementation

### Problem
Swarm (`swarm/mod.rs`) saves `SwarmSession` to `.scud/swarm/`, while Monitor/TUI (`spawn/monitor.rs`, `tui/app.rs`) expects `SpawnSession` in `.scud/spawn/`. Monitor ignores swarm → empty agents.

### Solution
Post-swarm execution, create `SpawnSession` proxy:
- `window_name = "task-{id}"` (matches spawn)
- Agents from `swarm_session.waves.flat_rounds.task_ids/parallel_tags`
- Titles/tags from `storage.load_tasks()`
- Save to `.scud/spawn/{session_name}.json`

TUI polls tmux status → detects Running/Completed.

### Changes
- `swarm/mod.rs`: Add `use crate::commands::spawn::monitor::{SpawnSession, save_session};`
- After wave loop (line ~525): `create_and_save_spawn_proxy(...)`
- Helper `create_spawn_proxy()`: reload phases → map id→(title,tag) → add_agent()

### Verification
- `scud swarm --limit 2` → `scud spawn monitor --session swarm-default` shows agents.
- `cargo test --features real-terminal`

### Tradeoffs
Quick bridge (no refactor). Future: Unified session format.

Date: 2026-01-20