# Swarm Feature Review

## Overall Assessment
The agent did a solid job porting and revamping the swarm feature to Go. Key strengths:
- Clean separation of concerns: wave planner, executor, backpressure/validation, attribution, rho integration.
- Good use of errgroup for parallel rounds with concurrency limit.
- Topological wave planning using Kahn's algorithm (robust against cycles).
- Configurable backpressure with auto-detection for common project types (Go, Rust, npm, Python).
- Smart repair fallback using sequential smart model with context from validation failures.
- Prompts are well-structured with task details, deps, guidance.
- Respects agent-driven status updates via `scud set-status`.
- Timeouts and context cancellation handled.
- UI feedback with headers, spinners, colors.

## Issues Found
1. **Attribution reliability**: `AttributeFailure` relies on `git blame` output containing `[TASK-ID]`. However, standard `git blame` output format (commit hash + author + line) rarely includes commit messages with task prefixes unless the code line itself has it. This means attributions often fail to match tasks.
   - Fix: Also scan validation error outputs directly for task ID patterns like `[1]`, `[TASK-5]`, etc.

2. **No explicit swarm session locking**: Old Rust tasks referenced preventing concurrent swarms via fs2 locks. Go uses storage-level flock for data files, but no tag-specific swarm lock to prevent multiple `scud swarm` instances running simultaneously on same tag.

3. **Repair mode complexity**: The recovery loop reloads state frequently and mixes agent repair with post-validation status setting. Could be simplified.

4. **Minor**: Some hardcoded defaults repeated in code and config.

## Changes Made
- Enhanced `AttributeFailure` to directly parse task IDs from validation error outputs (more reliable).
- Added task ID extraction helper.
- Updated tests.
- Minor cleanups.

The core functionality is production-ready for parallel AI agent task execution with safety gates. Attribution fix makes the backpressure/repair loop more effective.

Next steps if needed: Add swarm-level lock file, integrate adaptive timeouts from rho, add TUI progress monitor.

