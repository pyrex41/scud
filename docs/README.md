# SCUD Documentation

This directory contains all user-facing documentation for SCUD.

## Structure

```
docs/
├── README.md                     # This file
├── orchestrator.md               # Parallel execution patterns
├── reference/                    # Quick reference materials
│   ├── QUICK_REFERENCE.md        # Command cheat sheet
│   └── SCG_FORMAT_SPEC.md        # Task file format specification
└── features/                     # Feature-specific documentation
    └── PARALLEL_FEATURES.md      # Task locking & orchestration
```

## Documentation Overview

### For Users

- **Quick lookup:** [Quick Reference](reference/QUICK_REFERENCE.md) - Command cheat sheet
- **Parallel execution:** [Orchestrator Pattern](orchestrator.md) - Multi-agent workflows
- **Task file format:** [SCG Format Spec](reference/SCG_FORMAT_SPEC.md) - Task storage format
- **Task locking:** [Parallel Features](features/PARALLEL_FEATURES.md) - Claim/release mechanics

## Additional Resources

- **Main README**: [../README.md](../README.md)
- **Development Logs**: [../log_docs/](../log_docs/)
- **Test Documentation**: [../scud-cli/TESTING.md](../scud-cli/TESTING.md)
