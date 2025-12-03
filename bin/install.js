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
 * Prompt user for text input
 * @param {string} question - Question to ask
 * @returns {Promise<string>}
 */
function askText(question) {
  return new Promise((resolve) => {
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout
    });

    rl.question(`${question}: `, (answer) => {
      rl.close();
      resolve(answer.trim());
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
 * Copy SCUD slash command files from source scud/ directory to destination
 */
function copyScudCommands(src, dest) {
  if (!fs.existsSync(src)) {
    return false;
  }
  if (!fs.existsSync(dest)) {
    fs.mkdirSync(dest, { recursive: true });
  }

  // Copy all .md files from source to destination
  const files = fs.readdirSync(src).filter(f => f.endsWith('.md'));
  for (const file of files) {
    fs.copyFileSync(path.join(src, file), path.join(dest, file));
  }
  return files.length > 0;
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
    {
      name: 'xAI (Grok)',
      id: 'xai',
      model: 'grok-code-fast-1',
      env: 'XAI_API_KEY',
      models: ['grok-code-fast-1', 'grok-4-1-fast-reasoning', 'grok-4-1-fast', 'grok-3-fast']
    },
    {
      name: 'Anthropic (Claude)',
      id: 'anthropic',
      model: 'claude-sonnet-4-5-20250929',
      env: 'ANTHROPIC_API_KEY',
      models: ['claude-sonnet-4-5-20250929', 'claude-opus-4-5-20251101', 'claude-haiku-4-5-20251001', 'claude-opus-4-1-20250805']
    },
    {
      name: 'OpenAI (GPT)',
      id: 'openai',
      model: 'o3-mini',
      env: 'OPENAI_API_KEY',
      models: ['gpt-5.1', 'gpt-5.1-mini', 'o3-mini', 'o3', 'o4-mini', 'gpt-4.1']
    },
    {
      name: 'OpenRouter',
      id: 'openrouter',
      model: 'anthropic/claude-sonnet-4.5',
      env: 'OPENROUTER_API_KEY',
      models: ['anthropic/claude-sonnet-4.5', 'anthropic/claude-opus-4.5', 'openai/o3-mini', 'openai/gpt-4.1']
    },
  ];

  let selectedProvider;
  let selectedModel;

  if (flags.provider) {
    // --provider flag provided
    selectedProvider = providers.find(p => p.id === flags.provider.toLowerCase());
    if (!selectedProvider) {
      log(`⚠ Unknown provider: ${flags.provider}. Using default (xAI).`, 'yellow');
      selectedProvider = providers[0];
    }
    selectedModel = selectedProvider.model;
    log(`Using provider: ${selectedProvider.name} (--provider flag)`, 'blue');
  } else {
    // Interactive provider selection
    const providerIndex = await askSelection(
      'Select your AI provider:',
      providers.map(p => p.name),
      0
    );
    selectedProvider = providers[providerIndex];

    // Interactive model selection
    const modelOptions = [...selectedProvider.models, 'Custom (enter model name)'];
    const modelIndex = await askSelection(
      `Select model for ${selectedProvider.name}:`,
      modelOptions,
      0
    );

    if (modelIndex === modelOptions.length - 1) {
      // User selected "Custom"
      selectedModel = await askText('Enter model name');
      if (!selectedModel) {
        selectedModel = selectedProvider.model;
        log(`Using default model: ${selectedModel}`, 'yellow');
      }
    } else {
      selectedModel = selectedProvider.models[modelIndex];
    }
  }

  log(`✓ Provider: ${selectedProvider.name}`, 'green');
  log(`  Model: ${selectedModel}`, 'reset');
  log(`  Requires: export ${selectedProvider.env}=your-api-key`, 'yellow');

  // Create .scud directory
  log('\nStep 3: Creating SCUD structure...', 'blue');
  const scudDir = path.join(cwd, '.scud');
  const tasksDir = path.join(scudDir, 'tasks');
  const docsDir = path.join(scudDir, 'docs');

  if (!fs.existsSync(scudDir)) {
    fs.mkdirSync(scudDir, { recursive: true });
  }
  if (!fs.existsSync(tasksDir)) {
    fs.mkdirSync(tasksDir, { recursive: true });
  }
  if (!fs.existsSync(docsDir)) {
    fs.mkdirSync(docsDir, { recursive: true });
    log('✓ Created .scud/docs/', 'green');
  }

  // Create empty tasks.scg file (SCG format)
  const tasksFile = path.join(tasksDir, 'tasks.scg');
  if (!fs.existsSync(tasksFile)) {
    fs.writeFileSync(tasksFile, '');
    log('✓ Created tasks.scg', 'green');
  } else {
    log('✓ tasks.scg already exists', 'green');
  }

  // Create config file with selected provider and model
  const configFile = path.join(scudDir, 'config.toml');
  if (!fs.existsSync(configFile)) {
    const configContent = `[llm]
provider = "${selectedProvider.id}"
model = "${selectedModel}"
max_tokens = 4096
`;
    fs.writeFileSync(configFile, configContent);
    log('✓ Created config.toml', 'green');
  } else {
    log('✓ config.toml already exists', 'green');
  }

  // Create docs subdirectories inside .scud/docs/
  log('\nStep 4: Creating documentation directories...', 'blue');
  const docSubDirs = ['prd', 'phases', 'architecture', 'retrospectives'];
  docSubDirs.forEach(subdir => {
    const fullPath = path.join(docsDir, subdir);
    if (!fs.existsSync(fullPath)) {
      fs.mkdirSync(fullPath, { recursive: true });
    }
  });
  log('✓ Documentation directories created in .scud/docs/', 'green');

  // Step 5: Install SCUD slash commands
  log('\nStep 5: Installing SCUD slash commands...', 'blue');

  const packageRoot = path.join(__dirname, '..');

  // Claude Code commands (.claude/commands/scud/)
  const sourceClaudeScud = path.join(packageRoot, '.claude', 'commands', 'scud');
  const targetClaudeScud = path.join(cwd, '.claude', 'commands', 'scud');

  // OpenCode commands (.opencode/command/scud/)
  const sourceOpenCodeScud = path.join(packageRoot, '.opencode', 'command', 'scud');
  const targetOpenCodeScud = path.join(cwd, '.opencode', 'command', 'scud');

  const installedClaude = copyScudCommands(sourceClaudeScud, targetClaudeScud);
  const installedOpenCode = copyScudCommands(sourceOpenCodeScud, targetOpenCodeScud);

  if (installedClaude || installedOpenCode) {
    log('✓ Slash commands installed:', 'green');
    if (installedClaude) {
      log('  Claude Code: .claude/commands/scud/', 'blue');
    }
    if (installedOpenCode) {
      log('  OpenCode:    .opencode/command/scud/', 'blue');
    }
    log('  • /scud:task-list, /scud:task-next, /scud:task-show', 'reset');
    log('  • /scud:task-status, /scud:task-claim, /scud:task-stats', 'reset');
  } else {
    log('⚠ Could not find source commands to install', 'yellow');
  }

  // Create or update CLAUDE.md with agent instructions
  log('\nStep 6: Adding agent instructions to CLAUDE.md...', 'blue');
  const claudeMdPath = path.join(cwd, 'CLAUDE.md');
  const scudInstructions = `
## SCUD Task Management

This project uses SCUD (Sprint Cycle Unified Development) for task management.

### Session Workflow

1. **Start of session**: Run \`scud warmup\` to orient yourself
   - Shows current working directory and recent git history
   - Displays active tag, task counts, and any stale locks
   - Identifies the next available task

2. **Claim a task**: Use \`/scud:task-next\` or \`scud next --claim --name "Claude"\`
   - Always claim before starting work to prevent conflicts
   - Task context is stored in \`.scud/current-task\`

3. **Work on the task**: Implement the requirements
   - Reference task details with \`/scud:task-show <id>\`
   - Dependencies are automatically tracked by the DAG

4. **Commit with context**: Use \`scud commit -m "message"\` or \`scud commit -a -m "message"\`
   - Automatically prefixes commits with \`[TASK-ID]\`
   - Uses task title as default commit message if none provided

5. **Complete the task**: Mark done with \`/scud:task-status <id> done\`
   - The stop hook will prompt for task completion

### Progress Journaling

Keep a brief progress log during complex tasks:

\`\`\`
## Progress Log

### Session: 2025-01-15
- Investigated auth module, found issue in token refresh
- Updated refresh logic to handle edge case
- Tests passing, ready for review
\`\`\`

This helps maintain continuity across sessions and provides context for future work.

### Key Commands

- \`scud warmup\` - Session orientation
- \`scud next\` - Find next available task
- \`scud show <id>\` - View task details
- \`scud set-status <id> <status>\` - Update task status
- \`scud commit\` - Task-aware git commit
- \`scud stats\` - View completion statistics
`;

  if (fs.existsSync(claudeMdPath)) {
    const content = fs.readFileSync(claudeMdPath, 'utf8');
    if (!content.includes('## SCUD Task Management')) {
      fs.appendFileSync(claudeMdPath, scudInstructions);
      log('✓ Updated CLAUDE.md with SCUD instructions', 'green');
    } else {
      log('✓ CLAUDE.md already has SCUD instructions', 'green');
    }
  } else {
    fs.writeFileSync(claudeMdPath, `# Project Instructions\n${scudInstructions}`);
    log('✓ Created CLAUDE.md with SCUD instructions', 'green');
  }

  // Success message
  log('\n✅ SCUD initialized successfully!\n', 'green');
  log('Configuration:', 'blue');
  log(`  Provider: ${selectedProvider.name}`, 'reset');
  log(`  Model: ${selectedModel}`, 'reset');
  log('');
  log('Environment variable required:', 'yellow');
  log(`  export ${selectedProvider.env}=your-api-key`, 'reset');
  log('');
  log('Next steps:', 'blue');
  log(`  1. Set your API key: export ${selectedProvider.env}=your-key`);
  log('  2. Run: scud tags');
  log('  3. Start working: /scud:task-next (slash command)\n');
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
