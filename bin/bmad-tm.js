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

const commands = {
  init: 'Initialize BMAD-TM Lite in current project',
  status: 'Show current workflow status',
  install: 'Install slash commands for Claude Code',
  validate: 'Run workflow validation',
  help: 'Show this help message'
};

function showHelp() {
  console.log(`
╭─────────────────────╮
│                     │
│   BMAD-TM Lite CLI  │
│                     │
╰─────────────────────╯

Usage: bmad-tm <command> [options]

Commands:
  init          Initialize BMAD-TM Lite in current project
  install       Install slash commands for Claude Code
  status        Show current workflow status
  validate      Run workflow validation
  help          Show this help message

Examples:
  bmad-tm init                    # Initialize in current directory
  bmad-tm install --claude-code   # Install for Claude Code CLI
  bmad-tm install --project       # Install in project only
  bmad-tm status                  # Show workflow state
  bmad-tm validate                # Validate workflow state

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
