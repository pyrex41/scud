#!/usr/bin/env node

/**
 * BMAD-TM Lite CLI
 * Main entry point for bmad-tm commands
 */

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const command = process.argv[2];
const args = process.argv.slice(3);

// Task management commands (use built-in task manager)
const taskCommands = ['tags', 'use-tag', 'list', 'show', 'set-status', 'next', 'stats'];

// AI-powered commands (delegate to external task-master CLI)
const aiCommands = ['parse-prd', 'analyze-complexity', 'expand', 'research'];

const commands = {
  init: 'Initialize BMAD-TM Lite in current project',
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
╭─────────────────────╮
│                     │
│   BMAD-TM CLI       │
│                     │
╰─────────────────────╯

Usage: bmad-tm <command> [options]

Setup Commands:
  init          Initialize BMAD-TM in current project
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

AI-Powered (requires task-master CLI):
  For these features, use: task-master <command>

  parse-prd <file> --tag=<tag>    Parse PRD into tasks
  analyze-complexity              Analyze task complexity
  expand --id=<id>                Expand task into subtasks
  research "<query>"              AI research

Examples:
  bmad-tm init                       # Initialize in current directory
  bmad-tm tags                       # List all epics
  bmad-tm use-tag epic-1-auth        # Switch to epic
  bmad-tm next                       # Find next available task
  bmad-tm set-status 3 in-progress   # Start task 3

  task-master parse-prd epic.md --tag=epic-1   # Parse PRD (AI)
  task-master expand --id=5                    # Expand task (AI)

For more information, visit:
https://github.com/yourusername/bmad-tm-lite
`);
}

function init() {
  const installScript = path.join(__dirname, '..', 'bin', 'install.js');
  try {
    execSync(`node "${installScript}" init`, { stdio: 'inherit' });
  } catch (error) {
    console.error('Installation failed:', error.message);
    process.exit(1);
  }
}

function install() {
  const installScript = path.join(__dirname, '..', 'bin', 'install.js');
  const installArgs = args.join(' ');
  try {
    execSync(`node "${installScript}" ${installArgs}`, { stdio: 'inherit' });
  } catch (error) {
    console.error('Installation failed:', error.message);
    process.exit(1);
  }
}

function status() {
  const validator = path.join(__dirname, '..', 'src', 'validators', 'taskmaster-validator.js');
  try {
    const result = execSync(`node "${validator}" get-command-availability`, { encoding: 'utf8' });
    const availability = JSON.parse(result);

    console.log('\n📊 BMAD-TM Lite Workflow Status\n');
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
  try {
    execSync(`node "${validator}" validate-cli`, { stdio: 'inherit' });
    console.log('✅ Validation passed');
  } catch (error) {
    console.error('❌ Validation failed');
    process.exit(1);
  }
}

// Check if this is a task management command
if (taskCommands.includes(command)) {
  const taskManager = path.join(__dirname, '..', 'src', 'task-manager.js');
  try {
    execSync(`node "${taskManager}" ${command} ${args.join(' ')}`, { stdio: 'inherit' });
    process.exit(0);
  } catch (error) {
    process.exit(1);
  }
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
    console.log('Run "bmad-tm help" for usage information');
    process.exit(1);
}
