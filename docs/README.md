# SCUD Documentation

This directory contains all user-facing documentation for SCUD.

## Structure

```
docs/
├── README.md                     # This file
├── orchestrator.md               # Swarm execution, salvo worktrees, transcripts, SQLite
├── reference/                    # Quick reference materials
│   ├── QUICK_REFERENCE.md        # Command cheat sheet
│   └── SCG_FORMAT_SPEC.md        # Task file format specification
└── features/                     # Feature-specific documentation
    └── PARALLEL_FEATURES.md      # Task locking & orchestration
```

## Documentation Overview

### For Users

- **Quick lookup:** [Quick Reference](reference/QUICK_REFERENCE.md) - Command cheat sheet
- **Parallel execution:** [Orchestrator Pattern](orchestrator.md) - Swarm modes, salvo worktrees, transcript capture, SQLite storage
- **Task file format:** [SCG Format Spec](reference/SCG_FORMAT_SPEC.md) - Task storage format
- **Task locking:** [Parallel Features](features/PARALLEL_FEATURES.md) - Claim/release mechanics

### Swarm Execution

SCUD v1.47+ includes comprehensive swarm orchestration:

- **Wave mode** (default): Batch tasks into waves, validate between waves, repair on failure
- **Beads mode**: Continuous polling, spawn agents immediately when ready
- **Salvo worktrees**: Automatic git worktree provisioning per-tag for parallel isolation
- **SQLite storage**: All events, transcripts, and sessions stored in queryable `.scud/scud.db`
- **Transcript capture**: Real-time import of Claude Code conversation logs during swarm
- **Live monitoring**: Heartbeat detection, orphan detection, stale timeouts

See [Orchestrator Pattern](orchestrator.md) for detailed usage.

## Additional Resources

- **Main README**: [../README.md](../README.md)
- **Development Logs**: [../log_docs/](../log_docs/)
- **Test Documentation**: [../scud-cli/TESTING.md](../scud-cli/TESTING.md)
