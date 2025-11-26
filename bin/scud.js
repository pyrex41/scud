#!/usr/bin/env node

/**
 * SCUD CLI
 * Sprint Cycle Unified Development
 * Main entry point for scud commands
 */

const { execSync, spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const command = process.argv[2];
const args = process.argv.slice(3);

// Task management commands (use Rust CLI)
const taskCommands = ['tags', 'use-tag', 'list', 'show', 'set-status', 'next', 'stats'];

// AI-powered commands (use Rust CLI)
const aiCommands = ['parse-prd', 'analyze-complexity', 'expand', 'research'];

const versionCommands = ['--version', '-V'];

// All commands handled by Rust CLI
const rustCommands = [...taskCommands, ...aiCommands, ...versionCommands];

const commands = {
  init: 'Initialize SCUD in current project',
  status: 'Show current workflow status',
  install: 'Install slash commands for Claude Code',
  validate: 'Run workflow validation',
  help: 'Show this help message',
  // Task commands
  tags: 'List all epic tags',
  'use-tag': 'Switch to epic',
  list: 'List tasks in active epic',
  show: 'Show task details',
  'set-status': 'Update task status',
  next: 'Find next available task',
  stats: 'Show task statistics'
};

function showHelp() {
  console.log(`
╭────────────────────────────────────╮
│                                    │
│   SCUD CLI                         │
│   Sprint Cycle Unified Development │
│                                    │
╰────────────────────────────────────╯

Usage: scud <command> [options]

Setup Commands:
  init          Initialize SCUD in current project
  install       Install slash commands for Claude Code
  status        Show current workflow status
  validate      Run workflow validation

Task Management (built-in, fast):
  tags                        List all epic tags
  use-tag <tag>              Switch to epic
  list [--status=<status>]   List tasks in active epic
  show <id>                  Show task details
  set-status <id> <status>   Update task status
  next                       Find next available task
  stats                      Show task statistics

AI-Powered (built-in, requires ANTHROPIC_API_KEY):
  parse-prd <file> --tag=<tag>    Parse PRD into tasks
  analyze-complexity [--task=<id>] Analyze task complexity
  expand [<id>] [--all]           Expand task into subtasks
  research "<query>"              AI research

Examples:
  scud init                       # Initialize in current directory
  scud tags                       # List all epics
  scud use-tag epic-1-auth        # Switch to epic
  scud next                       # Find next available task
  scud set-status 3 in-progress   # Start task 3

  scud parse-prd epic.md --tag epic-1   # Parse PRD (AI)
  scud analyze-complexity               # Analyze all tasks (AI)
  scud expand --all                     # Expand complex tasks (AI)
  scud research "OAuth best practices"  # Research topic (AI)

For more information, visit:
https://github.com/yourusername/scud
`);
}

function init() {
  const installScript = path.join(__dirname, '..', 'bin', 'install.js');
  const result = spawnSync('node', [installScript, 'init'], { stdio: 'inherit' });
  if (result.status !== 0) {
    console.error('Installation failed');
    process.exit(1);
  }
}

function install() {
  const installScript = path.join(__dirname, '..', 'bin', 'install.js');
  const result = spawnSync('node', [installScript, ...args], { stdio: 'inherit' });
  if (result.status !== 0) {
    console.error('Installation failed');
    process.exit(1);
  }
}

function status() {
  const validator = path.join(__dirname, '..', 'src', 'validators', 'taskmaster-validator.js');
  const result = spawnSync('node', [validator, 'get-command-availability'], { encoding: 'utf8' });

  if (result.status !== 0) {
    console.error('Status check failed:', result.stderr);
    process.exit(1);
  }

  try {
    const availability = JSON.parse(result.stdout);

    console.log('\n📊 SCUD Workflow Status\n');
    console.log('Available Commands:');

    for (const [cmd, info] of Object.entries(availability)) {
      const icon = info.available ? '✅' : '❌';
      console.log(`  ${icon} /${cmd}`);
      console.log(`     ${info.reason}`);
    }
    console.log('');
  } catch (error) {
    console.error('Status check failed:', error.message);
    process.exit(1);
  }
}

function validate() {
  const validator = path.join(__dirname, '..', 'src', 'validators', 'taskmaster-validator.js');
  const result = spawnSync('node', [validator, 'validate-cli'], { stdio: 'inherit' });

  if (result.status === 0) {
    console.log('✅ Validation passed');
  } else {
    console.error('❌ Validation failed');
    process.exit(1);
  }
}

// Check if this is a command handled by Rust CLI
if (rustCommands.includes(command)) {
  // Find the Rust binary
  const rustBinary = path.join(__dirname, '..', 'scud-cli', 'target', 'release', 'scud');
  const debugBinary = path.join(__dirname, '..', 'scud-cli', 'target', 'debug', 'scud');

  // Use release binary if available, otherwise fall back to debug
  let scudBinary = fs.existsSync(rustBinary) ? rustBinary : debugBinary;

  if (!fs.existsSync(scudBinary)) {
    console.error('❌ SCUD Rust CLI not found. Building...');
    const scudCliDir = path.join(__dirname, '..', 'scud-cli');
    const buildResult = spawnSync('cargo', ['build', '--release'], { cwd: scudCliDir, stdio: 'inherit' });

    if (buildResult.status !== 0) {
      console.error('Failed to build Rust CLI. Please run: cd scud-cli && cargo build --release');
      process.exit(1);
    }
    scudBinary = rustBinary;
  }

  // Use spawnSync with argument array to properly handle spaces and special chars
  const result = spawnSync(scudBinary, [command, ...args], { stdio: 'inherit' });
  process.exit(result.status || 0);
}

switch (command) {
  case 'init':
    init();
    break;
  case 'install':
    install();
    break;
  case 'status':
    status();
    break;
  case 'validate':
    validate();
    break;
  case 'help':
  case undefined:
    showHelp();
    break;
  default:
    console.error(`Unknown command: ${command}`);
    console.log('Run "scud help" for usage information');
    process.exit(1);
}
