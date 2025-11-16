# SCUD Documentation

This directory contains all user-facing documentation for SCUD.

## Structure

```
docs/
├── README.md                     # This file
├── guides/                       # Getting started and comprehensive guides
│   ├── COMPLETE_GUIDE.md        # Full documentation (25,000 words)
│   └── MIGRATION.md             # Migration guide from BMAD-TM Lite
├── reference/                    # Quick reference materials
│   └── QUICK_REFERENCE.md       # Command cheat sheet
├── features/                     # Feature-specific documentation
│   └── PARALLEL_FEATURES.md     # Epic groups & task assignment
├── prd/                          # Product Requirements Documents
├── epics/                        # Epic descriptions
├── architecture/                 # Technical design documents
└── retrospectives/               # Project retrospectives and learnings
```

## Documentation Overview

### For Users

- **New to SCUD?** Start with [Complete Guide](guides/COMPLETE_GUIDE.md)
- **Upgrading?** See [Migration Guide](guides/MIGRATION.md)
- **Quick lookup?** Use [Quick Reference](reference/QUICK_REFERENCE.md)
- **Using parallel features?** See [Parallel Features](features/PARALLEL_FEATURES.md)

### For Development

- **PRDs**: Product requirements documents go in `prd/`
- **Epics**: Epic descriptions go in `epics/`
- **Architecture**: Technical designs go in `architecture/`
- **Retrospectives**: Post-epic learnings go in `retrospectives/`

## Additional Resources

- **Main README**: [../README.md](../README.md)
- **Development Logs**: [../log_docs/](../log_docs/)
- **Test Documentation**: [../scud-cli/TESTING.md](../scud-cli/TESTING.md)
