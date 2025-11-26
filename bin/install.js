#!/usr/bin/env node

/**
 * SCUD Installation Script
 * Handles initialization and setup in user projects
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const command = process.argv[2] || 'init';
const cwd = process.cwd();

// ANSI colors
const colors = {
  green: '\x1b[32m',
  blue: '\x1b[34m',
  yellow: '\x1b[33m',
  red: '\x1b[31m',
  reset: '\x1b[0m'
};

function log(message, color = 'reset') {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

function checkScud() {
  try {
    execSync('scud --version', { stdio: 'ignore' });
    return true;
  } catch {
    try {
      execSync('scud help', { stdio: 'ignore' });
      return true;
    } catch {
      return false;
    }
  }
}

function initProject() {
  log('\n🚀 Initializing SCUD in your project\n', 'blue');

  // Check SCUD CLI
  log('Step 1: Checking SCUD CLI...', 'blue');
  if (checkScud()) {
    log('✓ SCUD CLI found', 'green');
  } else {
    log('⚠ SCUD CLI not found (optional for AI features)', 'yellow');
  }

  // Create .scud directory
  log('\nStep 2: Creating SCUD structure...', 'blue');
  const scudDir = path.join(cwd, '.scud');
  const tasksDir = path.join(scudDir, 'tasks');

  if (!fs.existsSync(scudDir)) {
    fs.mkdirSync(scudDir, { recursive: true });
  }
  if (!fs.existsSync(tasksDir)) {
    fs.mkdirSync(tasksDir, { recursive: true });
  }

  // Create empty tasks.scg file (SCG format)
  const tasksFile = path.join(tasksDir, 'tasks.scg');
  if (!fs.existsSync(tasksFile)) {
    fs.writeFileSync(tasksFile, '');
    log('✓ Created tasks.scg', 'green');
  } else {
    log('✓ tasks.scg already exists', 'green');
  }

  const legacyTasksJson = path.join(tasksDir, 'tasks.json');
  if (!fs.existsSync(legacyTasksJson)) {
    fs.writeFileSync(legacyTasksJson, '{}');
    log('✓ Created tasks.json (legacy compatibility)', 'green');
  } else {
    log('✓ tasks.json already exists', 'green');
  }

  // Create workflow state
  log('\nStep 3: Creating workflow state...', 'blue');
  const workflowFile = path.join(scudDir, 'workflow-state.json');
  if (!fs.existsSync(workflowFile)) {
    const workflowState = {
      version: '1.0.0',
      current_phase: 'ideation',
      active_group: null,
      phases: {
        ideation: {
          status: 'active',
          completed_at: null,
          agent: 'scud-pm',
          description: 'Product definition and PRD creation'
        },
        planning: {
          status: 'pending',
          completed_at: null,
          agent: 'scud-sm',
          description: 'PRD parsing and task breakdown'
        },
        architecture: {
          status: 'pending',
          completed_at: null,
          agent: 'scud-architect',
          description: 'Technical design and architecture planning'
        },
        implementation: {
          status: 'pending',
          completed_at: null,
          agent: 'scud-dev',
          description: 'Task execution and development'
        },
        retrospective: {
          status: 'pending',
          completed_at: null,
          agent: 'scud-retrospective',
          description: 'Post-phase analysis and learning capture'
        }
      },
      history: [],
      completed_groups: [],
      last_updated: null
    };
    fs.writeFileSync(workflowFile, JSON.stringify(workflowState, null, 2));
    log('✓ Created workflow-state.json', 'green');
  } else {
    log('✓ workflow-state.json already exists', 'green');
  }

  // Create docs directories
  log('\nStep 4: Creating documentation directories...', 'blue');
  const docsDirs = ['docs/prd', 'docs/phases', 'docs/architecture', 'docs/retrospectives'];
  docsDirs.forEach(dir => {
    const fullPath = path.join(cwd, dir);
    if (!fs.existsSync(fullPath)) {
      fs.mkdirSync(fullPath, { recursive: true });
    }
  });
  log('✓ Documentation directories created', 'green');

  // Copy .claude commands
  log('\nStep 5: Installing slash commands...', 'blue');
  const packageRoot = path.join(__dirname, '..');
  const sourceCommands = path.join(packageRoot, '.claude');
  const targetCommands = path.join(cwd, '.claude');

  if (fs.existsSync(sourceCommands)) {
    copyDir(sourceCommands, targetCommands);
    log('✓ Slash commands installed to .claude/commands/', 'green');
    log('  • /status', 'blue');
    log('  • /scud-pm', 'blue');
    log('  • /scud-sm', 'blue');
    log('  • /scud-architect', 'blue');
    log('  • /scud-dev', 'blue');
    log('  • /scud-retrospective', 'blue');
  } else {
    log('⚠ Could not find source commands', 'yellow');
  }

  // Create .gitignore entry
  log('\nStep 6: Updating .gitignore...', 'blue');
  const gitignorePath = path.join(cwd, '.gitignore');
  const gitignoreEntry = '\n# SCUD\n.scud/\n';

  if (fs.existsSync(gitignorePath)) {
    const content = fs.readFileSync(gitignorePath, 'utf8');
    if (!content.includes('.scud/')) {
      fs.appendFileSync(gitignorePath, gitignoreEntry);
      log('✓ Updated .gitignore', 'green');
    } else {
      log('✓ .gitignore already configured', 'green');
    }
  } else {
    fs.writeFileSync(gitignorePath, gitignoreEntry);
    log('✓ Created .gitignore', 'green');
  }

  // Success message
  log('\n✅ SCUD initialized successfully!\n', 'green');
  log('Next steps:', 'blue');
  log('  1. Run: scud status');
  log('  2. Start with: /scud-pm (or use Claude Code slash command)\n');
}

function copyDir(src, dest) {
  if (!fs.existsSync(dest)) {
    fs.mkdirSync(dest, { recursive: true });
  }

  const entries = fs.readdirSync(src, { withFileTypes: true });

  for (const entry of entries) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);

    if (entry.isDirectory()) {
      copyDir(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

// Handle commands
switch (command) {
  case 'init':
    initProject();
    break;
  case '--claude-code':
    log('Installing for Claude Code CLI...', 'blue');
    log('⚠ Not yet implemented', 'yellow');
    break;
  case '--project':
    initProject();
    break;
  default:
    log(`Unknown command: ${command}`, 'red');
    process.exit(1);
}
