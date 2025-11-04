# BMAD-TM Lite - NPM Installation Guide

## Publishing to NPM

### Prerequisites

1. **NPM Account**
   ```bash
   npm login
   ```

2. **Update package.json**
   - Set your name in `author`
   - Set your repository URL
   - Choose a unique package name (check availability: `npm search bmad-tm-lite`)

### Publishing Steps

```bash
# 1. Test locally first
npm pack
# This creates a .tgz file you can test with:
# npm install ./bmad-tm-lite-1.0.0.tgz

# 2. Publish to npm
npm publish

# For scoped package (recommended for first publish):
npm publish --access public
```

### Package Name Options

If `bmad-tm-lite` is taken, consider:
- `@yourusername/bmad-tm-lite` (scoped package)
- `bmad-taskmaster-lite`
- `task-master-workflow`
- `bmad-workflow`

---

## Installing in Any Project

Once published, users can install with:

### Global Installation (recommended)

```bash
# Install globally for CLI commands
npm install -g bmad-tm-lite

# Initialize in any project
cd /path/to/your/project
bmad-tm init
```

### Local Installation (per-project)

```bash
# Install in project
npm install --save-dev bmad-tm-lite

# Initialize
npx bmad-tm init
```

---

## Usage After Installation

### 1. Initialize BMAD-TM Lite

```bash
bmad-tm init
```

This creates:
- `.taskmaster/` directory with workflow state
- `.claude/commands/` slash commands
- `docs/` directory structure

### 2. Check Status

```bash
bmad-tm status
```

Shows available commands and current phase.

### 3. Start Workflow (in Claude Code)

```bash
/tm-pm
```

Begin with Product Manager to create PRD.

---

## What Gets Installed

When `bmad-tm init` runs, it creates:

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

### bmad-tm init
Initializes BMAD-TM Lite in current project.

```bash
bmad-tm init
```

### bmad-tm status
Shows current workflow state and available commands.

```bash
bmad-tm status
```

### bmad-tm validate
Validates Task Master CLI installation.

```bash
bmad-tm validate
```

### bmad-tm help
Shows help information.

```bash
bmad-tm help
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
git clone https://github.com/yourusername/bmad-tm-lite.git
cd bmad-tm-lite

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
chmod +x node_modules/bmad-tm-lite/src/validators/taskmaster-validator.js

# Test it
node node_modules/bmad-tm-lite/src/validators/taskmaster-validator.js validate-cli
```

---

## Uninstallation

### Remove from Project

```bash
# Remove files
rm -rf .taskmaster .claude/commands

# Remove from package.json
npm uninstall bmad-tm-lite
```

### Global Uninstall

```bash
npm uninstall -g bmad-tm-lite
```

---

## Support

- **Issues**: https://github.com/yourusername/bmad-tm-lite/issues
- **Documentation**: See README.md
- **Examples**: See DETAILED_WALKTHROUGH.md

---

## License

MIT
