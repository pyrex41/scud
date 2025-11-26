#!/usr/bin/env node

/**
 * SCUD Installation Script
 * Handles initialization and setup in user projects
 */

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const readline = require('readline');

const cwd = process.cwd();

// Parse command line arguments
const args = process.argv.slice(2);
const command = args.find(arg => !arg.startsWith('--')) || 'init';
const flags = {
  agents: args.includes('--agents'),
  noAgents: args.includes('--no-agents'),
};

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

/**
 * Prompt user for yes/no confirmation
 * @param {string} question - Question to ask
 * @param {boolean} defaultYes - Default answer if user just presses Enter
 * @returns {Promise<boolean>}
 */
function askYesNo(question, defaultYes = true) {
  return new Promise((resolve) => {
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout
    });

    const hint = defaultYes ? '(Y/n)' : '(y/N)';
    rl.question(`${question} ${hint} `, (answer) => {
      rl.close();
      const normalized = answer.trim().toLowerCase();
      if (normalized === '') {
        resolve(defaultYes);
      } else {
        resolve(normalized === 'y' || normalized === 'yes');
      }
    });
  });
}

/**
 * Copy SCUD agent files from source scud/ directory to destination
 */
function copyScudAgents(src, dest) {
  if (!fs.existsSync(dest)) {
    fs.mkdirSync(dest, { recursive: true });
  }

  // Only copy the specific SCUD agent files
  const scudAgents = ['pm.md', 'sm.md', 'architect.md', 'dev.md', 'retrospective.md', 'status.md'];

  for (const agent of scudAgents) {
    const srcPath = path.join(src, agent);
    const destPath = path.join(dest, agent);

    if (fs.existsSync(srcPath)) {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

async function initProject() {
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

  // Step 5: Agent installation (interactive or flag-based)
  log('\nStep 5: SCUD Workflow Agents...', 'blue');

  let installAgents = false;

  if (flags.agents) {
    // --agents flag: install without prompting
    installAgents = true;
    log('Installing agents (--agents flag)', 'blue');
  } else if (flags.noAgents) {
    // --no-agents flag: skip without prompting
    installAgents = false;
    log('Skipping agents (--no-agents flag)', 'yellow');
  } else {
    // Interactive prompt
    log('');
    log('SCUD includes workflow agents for Claude Code:', 'blue');
    log('  • /scud-pm          - Product Manager (PRD creation)', 'reset');
    log('  • /scud-sm          - Scrum Master (task breakdown)', 'reset');
    log('  • /scud-architect   - Technical design', 'reset');
    log('  • /scud-dev         - Task implementation', 'reset');
    log('  • /scud-retrospective - Post-phase analysis', 'reset');
    log('  • /status           - Workflow status', 'reset');
    log('');

    installAgents = await askYesNo('Install SCUD workflow agents?', true);
  }

  const packageRoot = path.join(__dirname, '..');
  const sourceScud = path.join(packageRoot, '.claude', 'commands', 'scud');
  const targetScud = path.join(cwd, '.claude', 'commands', 'scud');

  if (installAgents) {
    if (fs.existsSync(sourceScud)) {
      // Copy SCUD agent files to .claude/commands/scud/
      copyScudAgents(sourceScud, targetScud);

      log('✓ Slash commands installed to .claude/commands/scud/', 'green');
      log('  • /scud-status', 'blue');
      log('  • /scud-pm', 'blue');
      log('  • /scud-sm', 'blue');
      log('  • /scud-architect', 'blue');
      log('  • /scud-dev', 'blue');
      log('  • /scud-retrospective', 'blue');
    } else {
      log('⚠ Could not find source commands at ' + sourceScud, 'yellow');
    }
  } else {
    log('⊘ Skipped agent installation', 'yellow');
    log('  You can add them later with: scud config agents add --all', 'reset');
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
  if (installAgents) {
    log('  2. Start with: /scud-pm (Claude Code slash command)\n');
  } else {
    log('  2. Add agents: scud config agents add --all');
    log('  3. Start with: /scud-pm (Claude Code slash command)\n');
  }
}

// Handle commands
switch (command) {
  case 'init':
    initProject().catch(err => {
      log(`Error: ${err.message}`, 'red');
      process.exit(1);
    });
    break;
  case '--claude-code':
    log('Installing for Claude Code CLI...', 'blue');
    log('⚠ Not yet implemented', 'yellow');
    break;
  case '--project':
    initProject().catch(err => {
      log(`Error: ${err.message}`, 'red');
      process.exit(1);
    });
    break;
  default:
    log(`Unknown command: ${command}`, 'red');
    process.exit(1);
}
