# SCUD Documentation

This directory contains all user-facing documentation for SCUD.

## Structure

```
docs/
├── README.md                     # This file
├── orchestrator.md               # Parallel execution patterns (spawn & swarm)
├── reference/                    # Quick reference materials
│   ├── QUICK_REFERENCE.md        # Command cheat sheet
│   └── SCG_FORMAT_SPEC.md        # Task file format specification
└── features/                     # Feature-specific documentation
    └── PARALLEL_FEATURES.md      # Task locking & orchestration
```

## Documentation Overview

### For Users

- **Quick lookup:** [Quick Reference](reference/QUICK_REFERENCE.md) - Command cheat sheet
- **Parallel execution:** [Orchestrator Pattern](orchestrator.md) - Swarm modes, spawn, and multi-agent workflows
- **Task file format:** [SCG Format Spec](reference/SCG_FORMAT_SPEC.md) - Task storage format
- **Task locking:** [Parallel Features](features/PARALLEL_FEATURES.md) - Claim/release mechanics

### Swarm Execution

SCUD v1.46+ includes two swarm execution modes:

- **Wave mode** (default): Batch tasks into waves, validate between waves
- **Beads mode**: Continuous polling, spawn agents immediately when ready

See [Orchestrator Pattern](orchestrator.md) for detailed usage.

## Additional Resources

- **Main README**: [../README.md](../README.md)
- **Development Logs**: [../log_docs/](../log_docs/)
- **Test Documentation**: [../scud-cli/TESTING.md](../scud-cli/TESTING.md)
