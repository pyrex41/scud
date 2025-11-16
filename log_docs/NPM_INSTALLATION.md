# SCUD - NPM Installation Guide
Sprint Cycle Unified Development

## Publishing to NPM

### Prerequisites

1. **NPM Account**
   ```bash
   npm login
   ```

2. **Update package.json**
   - Set your name in `author`
   - Set your repository URL
   - Choose a unique package name (check availability: `npm search scud`)

### Publishing Steps

```bash
# 1. Test locally first
npm pack
# This creates a .tgz file you can test with:
# npm install ./scud-1.0.0.tgz

# 2. Publish to npm
npm publish

# For scoped package (recommended for first publish):
npm publish --access public
```

### Package Name Options

If `scud` is taken, consider:
- `@yourusername/scud` (scoped package)
- `bmad-taskmaster-lite`
- `task-master-workflow`
- `bmad-workflow`

---

## Installing in Any Project

Once published, users can install with:

### Global Installation (recommended)

```bash
# Install globally for CLI commands
npm install -g scud

# Initialize in any project
cd /path/to/your/project
scud init
```

### Local Installation (per-project)

```bash
# Install in project
npm install --save-dev scud

# Initialize
npx scud init
```

---

## Usage After Installation

### 1. Initialize SCUD

```bash
scud init
```

This creates:
- `.taskmaster/` directory with workflow state
- `.claude/commands/` slash commands
- `docs/` directory structure

### 2. Check Status

```bash
scud status
```

Shows available commands and current phase.

### 3. Start Workflow (in Claude Code)

```bash
/tm-pm
```

Begin with Product Manager to create PRD.

---

## What Gets Installed

When `scud init` runs, it creates:

```
your-project/
├── .taskmaster/
│   ├── tasks/
│   │   └── tasks.json          # Task storage
│   └── workflow-state.json     # Workflow tracker
├── .claude/
│   └── commands/               # Slash commands
│       ├── status.md
│       ├── tm-pm.md
│       ├── tm-sm.md
│       ├── tm-architect.md
│       ├── tm-dev.md
│       └── tm-retrospective.md
├── docs/
│   ├── prd/                    # PRD documents
│   ├── epics/                  # Epic files
│   ├── architecture/           # Architecture docs
│   └── retrospectives/         # Retrospectives
└── .gitignore                  # Updated with .taskmaster/
```

---

## CLI Commands

### scud init
Initializes SCUD in current project.

```bash
scud init
```

### scud status
Shows current workflow state and available commands.

```bash
scud status
```

### scud validate
Validates Task Master CLI installation.

```bash
scud validate
```

### scud help
Shows help information.

```bash
scud help
```

---

## Integration with Claude Code

After initialization, all slash commands are available in `.claude/commands/`:

- `/status` - Show workflow state
- `/tm-pm` - Product Manager (PRD creation)
- `/tm-sm` - Scrum Master (Task Master operations)
- `/tm-architect` - Architect (Technical design)
- `/tm-dev` - Developer (Implementation)
- `/tm-retrospective` - Retrospective (Learning capture)

Claude Code automatically discovers these commands.

---

## Manual Installation (Without NPM)

If you prefer not to use npm:

```bash
# Clone the repository
git clone https://github.com/yourusername/scud.git
cd scud

# Copy to your project
cp -r .claude /path/to/your/project/
cp -r src /path/to/your/project/

# Run installation script
./install-claude-code.sh
```

---

## Dependencies

### Required
- **Task Master CLI**: `npm install -g task-master`
- **Node.js**: >= 16.0.0

### Optional
- **Claude Code**: For slash command integration

---

## Troubleshooting

### Task Master CLI Not Found

```bash
npm install -g task-master
```

### Commands Not Showing in Claude Code

1. Check `.claude/commands/` exists in project
2. Restart Claude Code
3. Try running `/status` to trigger discovery

### Validator Errors

```bash
# Make sure validator is executable
chmod +x node_modules/scud/src/validators/taskmaster-validator.js

# Test it
node node_modules/scud/src/validators/taskmaster-validator.js validate-cli
```

---

## Uninstallation

### Remove from Project

```bash
# Remove files
rm -rf .taskmaster .claude/commands

# Remove from package.json
npm uninstall scud
```

### Global Uninstall

```bash
npm uninstall -g scud
```

---

## Support

- **Issues**: https://github.com/yourusername/scud/issues
- **Documentation**: See README.md
- **Examples**: See DETAILED_WALKTHROUGH.md

---

## License

MIT
