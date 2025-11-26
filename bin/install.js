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
  provider: args.find(arg => arg.startsWith('--provider='))?.split('=')[1],
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
 * Prompt user for a numbered selection
 * @param {string} question - Question to ask
 * @param {string[]} options - Array of options
 * @param {number} defaultIndex - Default selection (0-indexed)
 * @returns {Promise<number>} - Selected index
 */
function askSelection(question, options, defaultIndex = 0) {
  return new Promise((resolve) => {
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout
    });

    log(`\n${question}`, 'blue');
    options.forEach((opt, i) => {
      const marker = i === defaultIndex ? '>' : ' ';
      log(`  ${marker} ${i + 1}) ${opt}`, 'reset');
    });

    rl.question(`\nSelect [1-${options.length}] (default: ${defaultIndex + 1}): `, (answer) => {
      rl.close();
      const normalized = answer.trim();
      if (normalized === '') {
        resolve(defaultIndex);
      } else {
        const num = parseInt(normalized, 10);
        if (num >= 1 && num <= options.length) {
          resolve(num - 1);
        } else {
          resolve(defaultIndex);
        }
      }
    });
  });
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
  const hasScudCli = checkScud();
  if (hasScudCli) {
    log('✓ SCUD CLI found', 'green');
  } else {
    log('⚠ SCUD CLI not found (optional for AI features)', 'yellow');
  }

  // Step 2: AI Provider selection
  log('\nStep 2: AI Provider Configuration...', 'blue');

  const providers = [
    { name: 'xAI (Grok)', id: 'xai', model: 'grok-3-fast-latest', env: 'XAI_API_KEY' },
    { name: 'Anthropic (Claude)', id: 'anthropic', model: 'claude-sonnet-4-20250514', env: 'ANTHROPIC_API_KEY' },
    { name: 'OpenAI (GPT)', id: 'openai', model: 'gpt-4.1', env: 'OPENAI_API_KEY' },
    { name: 'OpenRouter', id: 'openrouter', model: 'anthropic/claude-sonnet-4', env: 'OPENROUTER_API_KEY' },
  ];

  let selectedProvider;

  if (flags.provider) {
    // --provider flag provided
    selectedProvider = providers.find(p => p.id === flags.provider.toLowerCase());
    if (!selectedProvider) {
      log(`⚠ Unknown provider: ${flags.provider}. Using default (xAI).`, 'yellow');
      selectedProvider = providers[0];
    }
    log(`Using provider: ${selectedProvider.name} (--provider flag)`, 'blue');
  } else {
    // Interactive selection
    const providerIndex = await askSelection(
      'Select your AI provider for SCUD CLI features:',
      providers.map(p => p.name),
      0
    );
    selectedProvider = providers[providerIndex];
  }

  log(`✓ Provider: ${selectedProvider.name}`, 'green');
  log(`  Model: ${selectedProvider.model}`, 'reset');
  log(`  Requires: export ${selectedProvider.env}=your-api-key`, 'yellow');

  // Create .scud directory
  log('\nStep 3: Creating SCUD structure...', 'blue');
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

  // Create config file with selected provider
  const configFile = path.join(scudDir, 'config.toml');
  if (!fs.existsSync(configFile)) {
    const configContent = `[llm]
provider = "${selectedProvider.id}"
model = "${selectedProvider.model}"
max_tokens = 4096
`;
    fs.writeFileSync(configFile, configContent);
    log('✓ Created config.toml', 'green');
  } else {
    log('✓ config.toml already exists', 'green');
  }

  // Create workflow state
  log('\nStep 4: Creating workflow state...', 'blue');
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
  log('\nStep 5: Creating documentation directories...', 'blue');
  const docsDirs = ['docs/prd', 'docs/phases', 'docs/architecture', 'docs/retrospectives'];
  docsDirs.forEach(dir => {
    const fullPath = path.join(cwd, dir);
    if (!fs.existsSync(fullPath)) {
      fs.mkdirSync(fullPath, { recursive: true });
    }
  });
  log('✓ Documentation directories created', 'green');

  // Step 6: Agent installation (interactive or flag-based)
  log('\nStep 6: SCUD Workflow Agents...', 'blue');

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

  // Claude Code commands (.claude/commands/scud/)
  const sourceClaudeScud = path.join(packageRoot, '.claude', 'commands', 'scud');
  const targetClaudeScud = path.join(cwd, '.claude', 'commands', 'scud');

  // OpenCode commands (.opencode/command/scud/)
  const sourceOpenCodeScud = path.join(packageRoot, '.opencode', 'command', 'scud');
  const targetOpenCodeScud = path.join(cwd, '.opencode', 'command', 'scud');

  if (installAgents) {
    let installedClaude = false;
    let installedOpenCode = false;

    // Install Claude Code commands
    if (fs.existsSync(sourceClaudeScud)) {
      copyScudAgents(sourceClaudeScud, targetClaudeScud);
      installedClaude = true;
    }

    // Install OpenCode commands
    if (fs.existsSync(sourceOpenCodeScud)) {
      copyScudAgents(sourceOpenCodeScud, targetOpenCodeScud);
      installedOpenCode = true;
    }

    if (installedClaude || installedOpenCode) {
      log('✓ Slash commands installed:', 'green');
      if (installedClaude) {
        log('  Claude Code: .claude/commands/scud/', 'blue');
      }
      if (installedOpenCode) {
        log('  OpenCode:    .opencode/command/scud/', 'blue');
      }
      log('  • /scud:status', 'blue');
      log('  • /scud:pm', 'blue');
      log('  • /scud:sm', 'blue');
      log('  • /scud:architect', 'blue');
      log('  • /scud:dev', 'blue');
      log('  • /scud:retrospective', 'blue');
    } else {
      log('⚠ Could not find source commands', 'yellow');
    }
  } else {
    log('⊘ Skipped agent installation', 'yellow');
    log('  You can add them later with: scud config agents add --all', 'reset');
  }

  // Create .gitignore entry
  log('\nStep 7: Updating .gitignore...', 'blue');
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
  log('Configuration:', 'blue');
  log(`  Provider: ${selectedProvider.name}`, 'reset');
  log(`  Model: ${selectedProvider.model}`, 'reset');
  log('');
  log('Environment variable required:', 'yellow');
  log(`  export ${selectedProvider.env}=your-api-key`, 'reset');
  log('');
  log('Next steps:', 'blue');
  log(`  1. Set your API key: export ${selectedProvider.env}=your-key`);
  log('  2. Run: scud status');
  if (installAgents) {
    log('  3. Start with: /scud:pm (slash command)\n');
  } else {
    log('  3. Add agents: scud config agents add --all');
    log('  4. Start with: /scud:pm (slash command)\n');
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
